#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${PRIVACY_SDK_GUARD_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
NODE_OVERRIDE="${PRIVACY_SDK_GUARD_NODE_BIN:-}"
PYTHON_BIN="${PRIVACY_SDK_GUARD_PYTHON_BIN:-python3}"
SDK_PYTHON_OVERRIDE="${PRIVACY_PYTHON_SDK_PYTHON_BIN:-}"
VENV_DIR="${PRIVACY_SDK_GUARD_VENV:-${TMPDIR:-/tmp}/iroha-privacy-sdk-guard-venv}"
MODE="${1:-}"

node_candidate_path() {
  local candidate="$1"
  if command -v "${candidate}" >/dev/null 2>&1; then
    command -v "${candidate}"
    return 0
  fi
  if [[ -x "${candidate}" ]]; then
    printf '%s\n' "${candidate}"
    return 0
  fi
  return 1
}

is_node_20_bin() {
  local candidate="$1"
  local version
  version="$("${candidate}" --version 2>/dev/null || true)"
  [[ "${version}" == v20.* ]]
}

resolve_node_20_bin() {
  if [[ -n "${NODE_OVERRIDE}" ]]; then
    printf '%s\n' "${NODE_OVERRIDE}"
    return 0
  fi

  local candidate path
  for candidate in \
    node20 \
    node20.20 \
    /opt/homebrew/opt/node@20/bin/node \
    /usr/local/opt/node@20/bin/node \
    /opt/homebrew/Cellar/node@20/*/bin/node \
    /usr/local/Cellar/node@20/*/bin/node \
    "${HOME}"/.npm/_npx/*/node_modules/node/bin/node \
    node; do
    path="$(node_candidate_path "${candidate}" || true)"
    if [[ -n "${path}" ]] && is_node_20_bin "${path}"; then
      printf '%s\n' "${path}"
      return 0
    fi
  done

  printf '%s\n' "node"
}

resolve_python_311_bin() {
  if [[ -n "${SDK_PYTHON_OVERRIDE}" ]]; then
    printf '%s\n' "${SDK_PYTHON_OVERRIDE}"
    return 0
  fi

  local candidate
  for candidate in python3.11 /opt/homebrew/bin/python3.11 /usr/local/bin/python3.11 python3; do
    if command -v "${candidate}" >/dev/null 2>&1; then
      command -v "${candidate}"
      return 0
    fi
    if [[ -x "${candidate}" ]]; then
      printf '%s\n' "${candidate}"
      return 0
    fi
  done

  printf '%s\n' "python3"
}

"${PYTHON_BIN}" - "${ROOT_DIR}" "${MODE}" <<'PY'
import re
import subprocess
import sys
from fnmatch import fnmatchcase
from pathlib import Path

root = Path(sys.argv[1])
mode = sys.argv[2]
text_overrides = {}
workflow_path = ".github/workflows/pr_privacy_sdk_guard.yml"
bridge_header_command = "ci/check_connect_norito_bridge_header.sh"
bridge_header_drift_commands = (
    (
        "missing privacy C header declaration negative control",
        "ci/check_connect_norito_bridge_header.sh --negative-control-missing-privacy-header",
    ),
    (
        "bad privacy C header signature negative control",
        "ci/check_connect_norito_bridge_header.sh --negative-control-bad-privacy-signature",
    ),
    (
        "missing Rust privacy export negative control",
        "ci/check_connect_norito_bridge_header.sh --negative-control-missing-privacy-rust-export",
    ),
)
bytecode_command = "bash ci/check_no_tracked_python_bytecode.sh"
main_command = "ci/check_privacy_sdk_guard.sh"
native_bridge_command = "cargo test -p connect_norito_bridge privacy_ --lib -- --test-threads=1"
native_bridge_job = "privacy_native_bridge_tests"
csharp_sdk_command = "ci/check_privacy_csharp_sdk.sh"
csharp_sdk_job = "privacy_csharp_sdk_tests"
js_sdk_install_command = "npm ci --prefix javascript/iroha_js"
js_sdk_command = "ci/check_privacy_js_sdk.sh"
js_sdk_job = "privacy_javascript_sdk_tests"
jvm_sdk_command = "ci/check_privacy_jvm_sdk.sh"
jvm_sdk_job = "privacy_jvm_sdk_tests"
python_sdk_command = "ci/check_privacy_python_sdk.sh"
python_sdk_job = "privacy_python_sdk_tests"
swift_sdk_command = "ci/check_privacy_swift_sdk.sh"
swift_sdk_job = "privacy_swift_sdk_parse"
main_job = "privacy-sdk-guard"
main_job_needs_line = (
    "    needs: [privacy_native_bridge_tests, privacy_swift_sdk_parse, "
    "privacy_jvm_sdk_tests, privacy_csharp_sdk_tests, "
    "privacy_javascript_sdk_tests, privacy_python_sdk_tests]"
)
negative_control_commands = (
    (
        "bridge header workflow path negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-bridge-header-workflow-path",
    ),
    (
        "bridge header command workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-bridge-header-command-workflow",
    ),
    (
        "bridge header negative-control workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-bridge-header-negative-controls-workflow",
    ),
    (
        "tracked Python bytecode workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-bytecode-workflow",
    ),
    (
        "workflow path negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-workflow-path",
    ),
    (
        "workflow command negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-workflow-command",
    ),
    (
        "backend-tag workflow path negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-backend-tag-workflow-path",
    ),
    (
        "browser test workflow path negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-browser-test-workflow-path",
    ),
    (
        "negative-control workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-negative-controls-workflow",
    ),
    (
        "commented negative-control workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-negative-controls-comment-workflow",
    ),
    (
        "negative-control ordering workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-negative-controls-order-workflow",
    ),
    (
        "README boundary negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-readme-boundary",
    ),
    (
        "README API negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-readme-api",
    ),
    (
        "ZK-ACE proof-builder coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-zk-ace-proof-builder-coverage",
    ),
    (
        "Python catalog bytecode guard negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-catalog-bytecode-guard",
    ),
    (
        "README error-code negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-readme-error-code",
    ),
    (
        "browser source error-code negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-browser-error-code",
    ),
    (
        "browser dist error-code negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-browser-dist-error-code",
    ),
    (
        "workflow cancellation negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-workflow-cancel-in-progress",
    ),
    (
        "native bridge workflow job negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-native-bridge-job-workflow",
    ),
    (
        "native bridge runner workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-native-bridge-runner-workflow",
    ),
    (
        "native bridge test workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-native-bridge-test-workflow",
    ),
    (
        "native bridge dependency workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-native-bridge-needs-workflow",
    ),
    (
        "Swift SDK workflow job negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-swift-sdk-job-workflow",
    ),
    (
        "Swift SDK runner workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-swift-sdk-runner-workflow",
    ),
    (
        "Swift SDK parse workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-swift-sdk-parse-workflow",
    ),
    (
        "Swift SDK version script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-swift-sdk-version-script",
    ),
    (
        "Swift SDK compiler override script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-swift-sdk-override-script",
    ),
    (
        "Swift SDK dependency workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-swift-sdk-needs-workflow",
    ),
    (
        "JVM SDK workflow job negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-jvm-sdk-job-workflow",
    ),
    (
        "JVM SDK setup workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-jvm-sdk-setup-workflow",
    ),
    (
        "JVM SDK Java distribution workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-jvm-sdk-distribution-workflow",
    ),
    (
        "JVM SDK Java version workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-jvm-sdk-java-version-workflow",
    ),
    (
        "JVM SDK Java setup ordering workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-jvm-sdk-setup-order-workflow",
    ),
    (
        "JVM SDK test workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-jvm-sdk-test-workflow",
    ),
    (
        "JVM SDK JDK 21 script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-jvm-sdk-jdk21-script",
    ),
    (
        "JVM SDK Java home override script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-jvm-sdk-java-home-override-script",
    ),
    (
        "JVM SDK inherited Java home rejection script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-jvm-sdk-java-home-reject-script",
    ),
    (
        "JVM SDK dependency workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-jvm-sdk-needs-workflow",
    ),
    (
        "C# SDK workflow job negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-csharp-sdk-job-workflow",
    ),
    (
        "C# SDK setup workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-csharp-sdk-setup-workflow",
    ),
    (
        "C# SDK dotnet version workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-csharp-sdk-dotnet-version-workflow",
    ),
    (
        "C# SDK setup ordering workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-csharp-sdk-setup-order-workflow",
    ),
    (
        "C# SDK dotnet version script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-csharp-sdk-dotnet-version-script",
    ),
    (
        "C# SDK dotnet override script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-csharp-sdk-dotnet-override-script",
    ),
    (
        "C# SDK dotnet major script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-csharp-sdk-dotnet-major-script",
    ),
    (
        "C# SDK test workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-csharp-sdk-test-workflow",
    ),
    (
        "C# SDK dependency workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-csharp-sdk-needs-workflow",
    ),
    (
        "JavaScript SDK workflow job negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-js-sdk-job-workflow",
    ),
    (
        "JavaScript SDK runner workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-js-sdk-runner-workflow",
    ),
    (
        "JavaScript SDK Node setup workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-js-sdk-node-setup-workflow",
    ),
    (
        "JavaScript SDK Node version workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-js-sdk-node-version-workflow",
    ),
    (
        "JavaScript SDK Node version script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-js-sdk-node-version-script",
    ),
    (
        "JavaScript SDK Node override script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-js-sdk-node-override-script",
    ),
    (
        "JavaScript SDK Node resolver script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-js-sdk-node-resolver-script",
    ),
    (
        "JavaScript SDK Node major script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-js-sdk-node-major-script",
    ),
    (
        "JavaScript SDK Node cache workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-js-sdk-node-cache-workflow",
    ),
    (
        "JavaScript SDK Node setup ordering workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-js-sdk-node-setup-order-workflow",
    ),
    (
        "JavaScript SDK install workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-js-sdk-install-workflow",
    ),
    (
        "JavaScript SDK test workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-js-sdk-test-workflow",
    ),
    (
        "JavaScript SDK install ordering workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-js-sdk-install-order-workflow",
    ),
    (
        "JavaScript SDK test ordering workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-js-sdk-test-order-workflow",
    ),
    (
        "JavaScript SDK dependency workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-js-sdk-needs-workflow",
    ),
    (
        "Python SDK workflow job negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-sdk-job-workflow",
    ),
    (
        "Python SDK runner workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-sdk-runner-workflow",
    ),
    (
        "Python SDK setup workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-sdk-setup-workflow",
    ),
    (
        "Python SDK version workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-sdk-version-workflow",
    ),
    (
        "Python SDK setup ordering workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-sdk-setup-order-workflow",
    ),
    (
        "Python SDK Rust cache workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-sdk-rust-cache-workflow",
    ),
    (
        "Python SDK timeout workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-sdk-timeout-workflow",
    ),
    (
        "Python SDK version script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-sdk-version-script",
    ),
    (
        "Python SDK override script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-sdk-override-script",
    ),
    (
        "Python SDK resolver script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-sdk-resolver-script",
    ),
    (
        "Python SDK major script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-sdk-major-script",
    ),
    (
        "Python SDK stale venv rebuild script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-sdk-venv-rebuild-script",
    ),
    (
        "Python SDK native build script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-sdk-native-build-script",
    ),
    (
        "Python SDK venv activation script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-sdk-venv-activation-script",
    ),
    (
        "Python SDK bytecode script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-sdk-bytecode-script",
    ),
    (
        "Python SDK test workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-sdk-test-workflow",
    ),
    (
        "Python SDK dependency workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-sdk-needs-workflow",
    ),
    (
        "main guard Rust cache workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-main-rust-cache-workflow",
    ),
    (
        "main guard timeout workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-main-timeout-workflow",
    ),
)
required_paths = (
    workflow_path,
    "ci/check_connect_norito_bridge_header.sh",
    "ci/check_no_tracked_python_bytecode.sh",
    "ci/check_privacy_csharp_sdk.sh",
    "ci/check_privacy_js_sdk.sh",
    "ci/check_privacy_jvm_sdk.sh",
    "ci/check_privacy_python_sdk.sh",
    "ci/check_privacy_sdk_guard.sh",
    "ci/check_privacy_swift_sdk.sh",
    "crates/connect_norito_bridge/Cargo.toml",
    "crates/connect_norito_bridge/include/connect_norito_bridge.h",
    "crates/connect_norito_bridge/include/NoritoBridge.h",
    "crates/connect_norito_bridge/src/lib.rs",
    "crates/iroha_js_host/Cargo.toml",
    "crates/iroha_js_host/src/lib.rs",
    "crates/iroha_data_model/src/zk.rs",
    "python/iroha_python/iroha_python_rs/Cargo.toml",
    "python/iroha_python/iroha_python_rs/src/lib.rs",
    "javascript/iroha_js/src/crypto.js",
    "javascript/iroha_js/src/crypto.browser.js",
    "javascript/iroha_js/src/index.js",
    "javascript/iroha_js/src/instructionBuilders.js",
    "javascript/iroha_js/src/norito.js",
    "javascript/iroha_js/src/privacyAlgorithms.js",
    "javascript/iroha_js/dist/crypto.js",
    "javascript/iroha_js/dist/crypto.browser.js",
    "javascript/iroha_js/dist/index.js",
    "javascript/iroha_js/dist/instructionBuilders.js",
    "javascript/iroha_js/dist/norito.js",
    "javascript/iroha_js/dist/privacyAlgorithms.js",
    "javascript/iroha_js/index.d.ts",
    "javascript/iroha_js/README.md",
    "javascript/iroha_js/test/privacyFfiContractParity.test.js",
    "javascript/iroha_js/test/privacyCatalogParity.test.js",
    "javascript/iroha_js/test/privacyNative.test.js",
    "javascript/iroha_js/test/instructionBuilders.test.js",
    "javascript/iroha_js/test/package_dist.test.js",
    "javascript/iroha_js/test/crypto.browser.test.js",
    "python/iroha_python/src/iroha_python/__init__.py",
    "python/iroha_python/src/iroha_python/crypto.py",
    "python/iroha_python/src/iroha_python/privacy_catalog.py",
    "python/iroha_python/src/iroha_python/anonymous_pgc.py",
    "python/iroha_python/src/iroha_python/jindo.py",
    "python/iroha_python/src/iroha_python/silent_threshold.py",
    "python/iroha_python/src/iroha_python/sis_hints.py",
    "python/iroha_python/src/iroha_python/vega.py",
    "python/iroha_python/src/iroha_python/verange.py",
    "python/iroha_python/src/iroha_python/zk_ams.py",
    "python/iroha_python/src/iroha_python/zk_x509.py",
    "python/iroha_python/src/iroha_python/zkat.py",
    "python/iroha_python/tests/privacy_catalog_test.py",
    "python/iroha_python/tests/package_import_fallback_test.py",
    "python/iroha_python/tests/privacy_native_registry_test.py",
    "python/iroha_python/tests/crypto_algorithms_test.py",
    "python/iroha_python/tests/anonymous_pgc_test.py",
    "python/iroha_python/tests/jindo_test.py",
    "python/iroha_python/tests/silent_threshold_test.py",
    "python/iroha_python/tests/sis_hints_test.py",
    "python/iroha_python/tests/vega_test.py",
    "python/iroha_python/tests/verange_test.py",
    "python/iroha_python/tests/zk_ams_test.py",
    "python/iroha_python/tests/zk_x509_test.py",
    "python/iroha_python/tests/zkat_test.py",
    "python/iroha_python/README.md",
    "IrohaSwift/README.md",
    "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift",
    "IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift",
    "IrohaSwift/Sources/IrohaSwift/VerifyingKeyBackendTag.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/PrivacyNativeBridgeTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/VerifyingKeyBackendTagTests.swift",
    "java/iroha_android/README.md",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/instructions/RegisterPinManifestInstruction.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/instructions/VerifyingKeyInstructionUtils.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/zk/VerifyingKeyBackendTag.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/zk/VerifyingKeyRecordDescription.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/zk/VerifyingKeyStatus.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridge.java",
    "java/iroha_android/src/test/java/org/hyperledger/iroha/android/model/instructions/VerifyingKeyInstructionUtilsTests.java",
    "java/iroha_android/src/test/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridgeTest.java",
    "kotlin/README.md",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/core/model/instructions/RegisterPinManifestInstruction.kt",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/core/model/zk/VerifyingKeyBackendTag.kt",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridge.kt",
    "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/core/model/zk/VerifyingKeyBackendTagTest.kt",
    "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridgeTest.kt",
    "csharp/README.md",
    "csharp/src/Hyperledger.Iroha.Sdk/Hyperledger.Iroha.Sdk.csproj",
    "csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyNative.cs",
    "csharp/src/Hyperledger.Iroha.Sdk/Zk/VerifyingKeyBackendTag.cs",
    "csharp/tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj",
    "csharp/tests/Hyperledger.Iroha.Sdk.Tests/PrivacyNativeTests.cs",
    "csharp/tests/Hyperledger.Iroha.Sdk.Tests/VerifyingKeyBackendTagTests.cs",
    "docs/source/offline_kagemusha.md",
    "roadmap.md",
    "status.md",
)

readme_required = {
    "IrohaSwift/README.md": (
        "PrivacyNativeBridge",
        "capabilitiesV1()",
        "buildProofV1(requestArchive:)",
        "verifyProofV1(requestArchive:)",
        "productionReady = false",
        "ffiStatusError",
        "ffiErrorNullPointer",
        "ffiErrorMalformedNorito",
        "ffiErrorUnsupportedAlgorithm",
        "ffiErrorProductionDisabled",
        "ffiErrorInvalidRequest",
    ),
    "java/iroha_android/README.md": (
        "PrivacyNativeBridge",
        "capabilitiesArchive()",
        "buildProof(requestArchive)",
        "verifyProof(requestArchive)",
        "productionReady = false",
        "STATUS_ERROR",
        "ERROR_NULL_POINTER",
        "ERROR_MALFORMED_NORITO",
        "ERROR_UNSUPPORTED_ALGORITHM",
        "ERROR_PRODUCTION_DISABLED",
        "ERROR_INVALID_REQUEST",
    ),
    "kotlin/README.md": (
        "PrivacyNativeBridge",
        "capabilitiesArchive()",
        "buildProof(requestArchive)",
        "verifyProof(requestArchive)",
        "productionReady = false",
        "STATUS_ERROR",
        "ERROR_NULL_POINTER",
        "ERROR_MALFORMED_NORITO",
        "ERROR_UNSUPPORTED_ALGORITHM",
        "ERROR_PRODUCTION_DISABLED",
        "ERROR_INVALID_REQUEST",
    ),
    "csharp/README.md": (
        "Hyperledger.Iroha.Privacy.PrivacyNative",
        "CapabilitiesV1()",
        "BuildProofV1(requestArchive)",
        "VerifyProofV1(requestArchive)",
        "ProductionReady = false",
        "StatusError",
        "ErrorNullPointer",
        "ErrorMalformedNorito",
        "ErrorUnsupportedAlgorithm",
        "ErrorProductionDisabled",
        "ErrorInvalidRequest",
    ),
    "javascript/iroha_js/README.md": (
        "isPrivacyNativeAvailable()",
        "privacyCapabilitiesV1()",
        "privacyBuildProofV1(requestArchive)",
        "privacyVerifyProofV1(requestArchive)",
        "productionReady = false",
        "PRIVACY_FFI_STATUS_ERROR",
        "PRIVACY_FFI_ERROR_NULL_POINTER",
        "PRIVACY_FFI_ERROR_MALFORMED_NORITO",
        "PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM",
        "PRIVACY_FFI_ERROR_PRODUCTION_DISABLED",
        "PRIVACY_FFI_ERROR_INVALID_REQUEST",
    ),
    "python/iroha_python/README.md": (
        "privacy_capabilities_v1()",
        "privacy_build_proof_v1(request_archive)",
        "privacy_verify_proof_v1(request_archive)",
        "build_zk_ace_authorization_proof_v1()",
        "zk_ace_build_transfer_authorization_v1()",
        "production_ready = False",
        "PRIVACY_FFI_STATUS_ERROR",
        "PRIVACY_FFI_ERROR_NULL_POINTER",
        "PRIVACY_FFI_ERROR_MALFORMED_NORITO",
        "PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM",
        "PRIVACY_FFI_ERROR_PRODUCTION_DISABLED",
        "PRIVACY_FFI_ERROR_INVALID_REQUEST",
    ),
}

common_readme_required = (
    "raw Norito archives",
    "64 MiB native size cap",
    "operation-specific result schema",
    "privacy-production-gate-v1",
    "fail-closed",
    "capabilities",
    "build",
    "verify",
    "deterministic privacy FFI status/error-code contract",
    "status_error = 1",
    "null_pointer = 1",
    "malformed_norito = 2",
    "unsupported_algorithm = 3",
    "production_disabled = 4",
    "invalid_request = 5",
    "sanitized status metadata, not proof success",
)


class PrivacyGuardError(RuntimeError):
    pass


def read(relative):
    if relative in text_overrides:
        return text_overrides[relative]
    return (root / relative).read_text(encoding="utf-8")


def require(condition, message, errors):
    if not condition:
        errors.append(message)


def workflow_trigger_paths():
    paths = []
    in_paths = False
    for line in read(workflow_path).splitlines():
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


def check_readmes(errors):
    for relative, language_needles in readme_required.items():
        text = " ".join(read(relative).split())
        missing_needles = [
            needle for needle in (*common_readme_required, *language_needles) if needle not in text
        ]
        require(
            not missing_needles,
            (
                f"{relative} is missing privacy native bridge documentation tokens: "
                + ", ".join(missing_needles)
            ),
            errors,
        )


def check_zk_ace_proof_builder_coverage(errors):
    js_parity = read("javascript/iroha_js/test/privacyCatalogParity.test.js")
    python_catalog = read("python/iroha_python/src/iroha_python/privacy_catalog.py")
    python_crypto = read("python/iroha_python/src/iroha_python/crypto.py")
    python_package_root = read("python/iroha_python/src/iroha_python/__init__.py")
    python_tests = read("python/iroha_python/tests/privacy_catalog_test.py")
    python_readme = read("python/iroha_python/README.md")

    require(
        "function assertPythonZkAceProofBuilderCoverage()" in js_parity,
        "Privacy catalog parity tests must define the Python ZK-ACE proof-builder coverage guard",
        errors,
    )
    require(
        re.search(r"(?m)^\s*assertPythonZkAceProofBuilderCoverage\(\);\s*$", js_parity)
        is not None,
        "Privacy catalog parity tests must invoke the Python ZK-ACE proof-builder coverage guard",
        errors,
    )
    require(
        'env: { ...process.env, PYTHONDONTWRITEBYTECODE: "1" }' in js_parity,
        "Privacy catalog parity tests must suppress Python bytecode when loading the Python catalog",
        errors,
    )
    for snippet in (
        "Python privacy capabilities must require both ZK-ACE proof-builder names",
        "Python catalog-named ZK-ACE proof builder must delegate to the native-backed builder",
        "Python tests must cover ZK-ACE alias delegation, missing-native propagation, and malformed native prover payloads",
        "privacy algorithm catalogs pin executable ZK-ACE proof-builder descriptor shape",
        "assertZkAceExecutableDescriptorShape",
        "buildShieldedZkAceAuthorizedTransferInstruction",
        "planned shielded SDK entrypoints",
    ):
        require(
            snippet in js_parity,
            f"Privacy catalog parity tests are missing ZK-ACE proof-builder assertion: {snippet}",
            errors,
        )
    require(
        re.search(
            r'zk_ace_prover\s*=\s*_callable_on_crypto\(\s*"build_zk_ace_authorization_proof_v1"\s*\)\s*and\s*_callable_on_crypto\("zk_ace_build_transfer_authorization_v1"\)',
            python_catalog,
        )
        is not None,
        "Python privacy capabilities must fail closed unless both ZK-ACE proof-builder names are callable",
        errors,
    )
    require(
        re.search(
            r"def\s+build_zk_ace_authorization_proof_v1\(\*\*kwargs:\s*Any\)\s*->\s*Dict\[str,\s*Any\]:[\s\S]*return\s+zk_ace_build_transfer_authorization_v1\(\*\*kwargs\)",
            python_crypto,
        )
        is not None,
        "Python catalog-named ZK-ACE proof builder must delegate to the native-backed alias",
        errors,
    )
    for label, text in (
        ("Python crypto exports", python_crypto),
        ("Python package root exports", python_package_root),
        ("Python README", python_readme),
    ):
        require(
            "build_zk_ace_authorization_proof_v1" in text
            and "zk_ace_build_transfer_authorization_v1" in text,
            f"{label} must keep both ZK-ACE proof-builder names",
            errors,
        )
    for snippet in (
        "def test_zk_ace_python_capabilities_require_both_proof_builder_names(",
        '"build_zk_ace_authorization_proof_v1"',
        '"zk_ace_build_transfer_authorization_v1"',
        'assert capabilities["zk_ace_authorization_proof_v1"] is False',
        'assert capabilities["zk_ace_sdk_exports_v1"] is False',
        "test_zk_ace_python_catalog_named_proof_builder_delegates",
        "test_zk_ace_python_catalog_named_proof_builder_propagates_native_errors",
        "test_zk_ace_python_transfer_authorization_rejects_non_object_native_payload",
    ):
        require(
            snippet in python_tests,
            f"Python ZK-ACE proof-builder adversarial coverage is missing {snippet}",
            errors,
        )


def check_workflow_paths(errors):
    paths = workflow_trigger_paths()
    missing = [
        relative
        for relative in required_paths
        if not any(fnmatchcase(relative, pattern) for pattern in paths)
    ]
    require(
        not missing,
        "Privacy SDK guard workflow paths do not cover required sources: "
        + ", ".join(missing),
        errors,
    )


def check_workflow_commands(errors):
    workflow = read(workflow_path)
    main_match = re.search(rf"(?m)^\s+run:\s+{re.escape(main_command)}\s*$", workflow)
    bridge_header_match = workflow_command_match(workflow, bridge_header_command)
    bytecode_match = workflow_command_match(workflow, bytecode_command)
    main_job_block = workflow_job_block(workflow, main_job)
    require(
        bridge_header_match is not None,
        "Privacy SDK guard workflow must run ci/check_connect_norito_bridge_header.sh",
        errors,
    )
    require(
        main_match is not None,
        "Privacy SDK guard workflow must run ci/check_privacy_sdk_guard.sh",
        errors,
    )
    require(
        bytecode_match is not None,
        "Privacy SDK guard workflow must reject tracked Python bytecode artifacts",
        errors,
    )
    if main_match is None:
        return
    require(
        main_job_block is not None,
        "Privacy SDK guard workflow must define the aggregate guard job",
        errors,
    )
    if main_job_block is not None:
        require(
            re.search(r"(?m)^\s+timeout-minutes:\s+45\s*$", main_job_block) is not None,
            "Privacy SDK guard workflow must allow 45 minutes for native Python builds",
            errors,
        )
        require(
            re.search(r"(?m)^\s+- uses:\s+Swatinem/rust-cache@v2\s*$", main_job_block)
            is not None,
            "Privacy SDK guard workflow must cache Rust artifacts for aggregate native Python builds",
            errors,
        )
    if bridge_header_match is not None:
        require(
            bridge_header_match.start() < main_match.start(),
            "Privacy SDK guard workflow must run the bridge header parity check before the main guard",
            errors,
        )
    for label, command in bridge_header_drift_commands:
        command_match = workflow_command_match(workflow, command)
        require(
            command_match is not None,
            f"Privacy SDK guard workflow must run the bridge header {label}",
            errors,
        )
        if command_match is not None:
            require(
                command_match.start() < main_match.start(),
                f"Privacy SDK guard workflow must run the bridge header {label} before the main guard",
                errors,
            )
    if bytecode_match is not None:
        require(
            bytecode_match.start() < main_match.start(),
            "Privacy SDK guard workflow must reject tracked Python bytecode before the main guard",
            errors,
        )
    for label, command in negative_control_commands:
        command_match = workflow_command_match(workflow, command)
        require(
            command_match is not None,
            f"Privacy SDK guard workflow must run the {label}",
            errors,
        )
        if command_match is not None:
            require(
                command_match.start() < main_match.start(),
                f"Privacy SDK guard workflow must run the {label} before the main guard",
                errors,
            )


def check_workflow_cancellation_policy(errors):
    workflow = read(workflow_path)
    require(
        re.search(r"(?m)^\s+cancel-in-progress:\s+false\s*$", workflow) is not None,
        "Privacy SDK guard workflow must keep cancel-in-progress disabled",
        errors,
    )


def check_workflow_runs_native_bridge_tests(errors):
    workflow = read(workflow_path)
    job_block = workflow_job_block(workflow, native_bridge_job)
    main_needs = workflow_job_needs(workflow, main_job)
    require(
        job_block is not None,
        "Privacy SDK guard workflow must define the native bridge test job",
        errors,
    )
    if job_block is None:
        return
    require(
        re.search(r"(?m)^\s+runs-on:\s+ubuntu-latest\s*$", job_block) is not None,
        "Privacy SDK guard workflow must run native bridge tests on Ubuntu",
        errors,
    )
    require(
        re.search(r"(?m)^\s+- uses:\s+Swatinem/rust-cache@v2\s*$", job_block) is not None,
        "Privacy SDK guard workflow must cache Rust artifacts for native bridge tests",
        errors,
    )
    require(
        workflow_command_match(job_block, native_bridge_command) is not None,
        "Privacy SDK guard workflow must run the native privacy bridge tests",
        errors,
    )
    require(
        native_bridge_job in main_needs,
        "Privacy SDK guard job must wait for the native bridge test job",
        errors,
    )


def check_workflow_runs_csharp_sdk_tests(errors):
    workflow = read(workflow_path)
    job_block = workflow_job_block(workflow, csharp_sdk_job)
    main_needs = workflow_job_needs(workflow, main_job)
    require(
        job_block is not None,
        "Privacy SDK guard workflow must define the C# SDK test job",
        errors,
    )
    if job_block is None:
        return
    dotnet_setup_match = re.search(r"(?m)^\s+- uses:\s+actions/setup-dotnet@v4\s*$", job_block)
    dotnet_version_match = re.search(r"(?m)^\s+dotnet-version:\s+8\.0\.x\s*$", job_block)
    command_match = workflow_command_match(job_block, csharp_sdk_command)
    require(
        dotnet_setup_match is not None,
        "Privacy SDK guard workflow must set up dotnet for C# SDK tests",
        errors,
    )
    require(
        dotnet_version_match is not None,
        "Privacy SDK guard workflow must pin dotnet 8 for C# SDK tests",
        errors,
    )
    require(
        command_match is not None,
        "Privacy SDK guard workflow must run the C# privacy SDK tests",
        errors,
    )
    if dotnet_setup_match is not None and command_match is not None:
        require(
            dotnet_setup_match.start() < command_match.start(),
            "Privacy SDK guard workflow must set up dotnet before running C# SDK tests",
            errors,
        )
    require(
        csharp_sdk_job in main_needs,
        "Privacy SDK guard job must wait for the C# SDK test job",
        errors,
    )


def check_csharp_sdk_script_prints_dotnet_version(errors):
    script = read(csharp_sdk_command)
    require(
        'DOTNET_BIN="${PRIVACY_CSHARP_DOTNET_BIN:-dotnet}"' in script,
        "Privacy C# SDK script must keep the documented dotnet override variable",
        errors,
    )
    require(
        'DOTNET_VERSION="$("${DOTNET_BIN}" --version)"' in script,
        "Privacy C# SDK script must print the selected dotnet version",
        errors,
    )
    require(
        'printf \'%s\\n\' "${DOTNET_VERSION}"' in script,
        "Privacy C# SDK script must emit the selected dotnet version",
        errors,
    )
    require(
        "8.0.*) ;;" in script,
        "Privacy C# SDK script must reject non-.NET-8 SDK versions",
        errors,
    )


def check_workflow_runs_swift_sdk_parse(errors):
    workflow = read(workflow_path)
    job_block = workflow_job_block(workflow, swift_sdk_job)
    main_needs = workflow_job_needs(workflow, main_job)
    require(
        job_block is not None,
        "Privacy SDK guard workflow must define the Swift SDK parse job",
        errors,
    )
    if job_block is None:
        return
    require(
        re.search(r"(?m)^\s+runs-on:\s+macos-latest\s*$", job_block) is not None,
        "Privacy SDK guard workflow must run Swift SDK parsing on macOS",
        errors,
    )
    require(
        workflow_command_match(job_block, swift_sdk_command) is not None,
        "Privacy SDK guard workflow must run the Swift privacy SDK parse check",
        errors,
    )
    require(
        swift_sdk_job in main_needs,
        "Privacy SDK guard job must wait for the Swift SDK parse job",
        errors,
    )


def check_swift_sdk_script_prints_swiftc_version(errors):
    script = read(swift_sdk_command)
    require(
        'SWIFTC_BIN="${PRIVACY_SWIFT_SDK_SWIFTC_BIN:-swiftc}"' in script,
        "Privacy Swift SDK script must keep the documented swiftc override variable",
        errors,
    )
    require(
        '"${SWIFTC_BIN}" --version' in script,
        "Privacy Swift SDK script must print the selected swiftc version",
        errors,
    )


def check_workflow_runs_jvm_sdk_tests(errors):
    workflow = read(workflow_path)
    job_block = workflow_job_block(workflow, jvm_sdk_job)
    main_needs = workflow_job_needs(workflow, main_job)
    require(
        job_block is not None,
        "Privacy SDK guard workflow must define the JVM SDK test job",
        errors,
    )
    if job_block is None:
        return
    java_setup_match = re.search(r"(?m)^\s+- uses:\s+actions/setup-java@v4\s*$", job_block)
    java_distribution_match = re.search(r'(?m)^\s+distribution:\s+"temurin"\s*$', job_block)
    java_version_match = re.search(r'(?m)^\s+java-version:\s+"21"\s*$', job_block)
    command_match = workflow_command_match(job_block, jvm_sdk_command)
    require(
        java_setup_match is not None,
        "Privacy SDK guard workflow must set up Java for JVM SDK tests",
        errors,
    )
    require(
        java_distribution_match is not None,
        "Privacy SDK guard workflow must pin Temurin for JVM SDK tests",
        errors,
    )
    require(
        java_version_match is not None,
        "Privacy SDK guard workflow must pin Java 21 for JVM SDK tests",
        errors,
    )
    require(
        command_match is not None,
        "Privacy SDK guard workflow must run the JVM privacy SDK tests",
        errors,
    )
    if java_setup_match is not None and command_match is not None:
        require(
            java_setup_match.start() < command_match.start(),
            "Privacy SDK guard workflow must set up Java before running JVM SDK tests",
            errors,
        )
    require(
        jvm_sdk_job in main_needs,
        "Privacy SDK guard job must wait for the JVM SDK test job",
        errors,
    )


def check_workflow_runs_javascript_sdk_tests(errors):
    workflow = read(workflow_path)
    job_block = workflow_job_block(workflow, js_sdk_job)
    main_needs = workflow_job_needs(workflow, main_job)
    require(
        job_block is not None,
        "Privacy SDK guard workflow must define the JavaScript SDK test job",
        errors,
    )
    if job_block is None:
        return
    setup_match = re.search(r"(?m)^\s+- uses:\s+actions/setup-node@v4\s*$", job_block)
    install_match = workflow_command_match(job_block, js_sdk_install_command)
    test_match = workflow_command_match(job_block, js_sdk_command)
    require(
        re.search(r"(?m)^\s+runs-on:\s+ubuntu-latest\s*$", job_block) is not None,
        "Privacy SDK guard workflow must run JavaScript SDK tests on Ubuntu",
        errors,
    )
    require(
        setup_match is not None,
        "Privacy SDK guard workflow must set up Node for JavaScript SDK tests",
        errors,
    )
    require(
        re.search(r'(?m)^\s+node-version:\s+"20"\s*$', job_block) is not None,
        "Privacy SDK guard workflow must pin Node 20 for JavaScript SDK tests",
        errors,
    )
    require(
        re.search(
            r"(?m)^\s+cache-dependency-path:\s+javascript/iroha_js/package-lock\.json\s*$",
            job_block,
        )
        is not None,
        "Privacy SDK guard workflow must cache JavaScript SDK dependencies by package-lock",
        errors,
    )
    require(
        install_match is not None,
        "Privacy SDK guard workflow must install JavaScript SDK dependencies",
        errors,
    )
    require(
        test_match is not None,
        "Privacy SDK guard workflow must run the JavaScript privacy SDK tests",
        errors,
    )
    if setup_match is not None and install_match is not None:
        require(
            setup_match.start() < install_match.start(),
            "Privacy SDK guard workflow must set up Node before installing JavaScript SDK dependencies",
            errors,
        )
    if install_match is not None and test_match is not None:
        require(
            install_match.start() < test_match.start(),
            "Privacy SDK guard workflow must install JavaScript SDK dependencies before running JavaScript tests",
            errors,
        )
    require(
        js_sdk_job in main_needs,
        "Privacy SDK guard job must wait for the JavaScript SDK test job",
        errors,
    )


def check_javascript_sdk_script_prints_node_version(errors):
    script = read(js_sdk_command)
    require(
        'NODE_OVERRIDE="${PRIVACY_JS_SDK_NODE_BIN:-}"' in script,
        "Privacy JavaScript SDK script must keep the documented Node override variable",
        errors,
    )
    require(
        "resolve_node_20_bin()" in script and "is_node_20_bin()" in script,
        "Privacy JavaScript SDK script must resolve Node 20 before falling back to node",
        errors,
    )
    require(
        'NODE_BIN="$(resolve_node_20_bin)"' in script,
        "Privacy JavaScript SDK script must use the Node 20 resolver",
        errors,
    )
    require(
        'NODE_VERSION="$("${NODE_BIN}" --version)"' in script,
        "Privacy JavaScript SDK script must print the selected Node version",
        errors,
    )
    require(
        'printf \'%s\\n\' "${NODE_VERSION}"' in script,
        "Privacy JavaScript SDK script must emit the selected Node version",
        errors,
    )
    require(
        "v20.*) ;;" in script,
        "Privacy JavaScript SDK script must reject non-Node-20 runtimes",
        errors,
    )


def check_workflow_runs_python_sdk_tests(errors):
    workflow = read(workflow_path)
    job_block = workflow_job_block(workflow, python_sdk_job)
    main_needs = workflow_job_needs(workflow, main_job)
    require(
        job_block is not None,
        "Privacy SDK guard workflow must define the Python SDK test job",
        errors,
    )
    if job_block is None:
        return
    setup_match = re.search(r"(?m)^\s+- uses:\s+actions/setup-python@v5\s*$", job_block)
    test_match = workflow_command_match(job_block, python_sdk_command)
    require(
        re.search(r"(?m)^\s+runs-on:\s+ubuntu-latest\s*$", job_block) is not None,
        "Privacy SDK guard workflow must run Python SDK tests on Ubuntu",
        errors,
    )
    require(
        re.search(r"(?m)^\s+timeout-minutes:\s+45\s*$", job_block) is not None,
        "Privacy SDK guard workflow must allow 45 minutes for Python native builds",
        errors,
    )
    require(
        re.search(r"(?m)^\s+- uses:\s+Swatinem/rust-cache@v2\s*$", job_block) is not None,
        "Privacy SDK guard workflow must cache Rust artifacts for Python native builds",
        errors,
    )
    require(
        setup_match is not None,
        "Privacy SDK guard workflow must set up Python for Python SDK tests",
        errors,
    )
    require(
        re.search(r'(?m)^\s+python-version:\s+"3\.11"\s*$', job_block) is not None,
        "Privacy SDK guard workflow must pin Python 3.11 for Python SDK tests",
        errors,
    )
    require(
        test_match is not None,
        "Privacy SDK guard workflow must run the Python privacy SDK tests",
        errors,
    )
    if setup_match is not None and test_match is not None:
        require(
            setup_match.start() < test_match.start(),
            "Privacy SDK guard workflow must set up Python before running Python SDK tests",
            errors,
        )
    require(
        python_sdk_job in main_needs,
        "Privacy SDK guard job must wait for the Python SDK test job",
        errors,
    )


def check_python_sdk_script_prints_python_version(errors):
    script = read(python_sdk_command)
    require(
        'PYTHON_OVERRIDE="${PRIVACY_PYTHON_SDK_PYTHON_BIN:-}"' in script,
        "Privacy Python SDK script must keep the documented Python override variable",
        errors,
    )
    require(
        "resolve_python_311_bin()" in script and "python3.11" in script,
        "Privacy Python SDK script must resolve Python 3.11 before falling back to python3",
        errors,
    )
    require(
        'PYTHON_BIN="$(resolve_python_311_bin)"' in script,
        "Privacy Python SDK script must use the Python 3.11 resolver",
        errors,
    )
    require(
        'PYTHON_VERSION="$("${PYTHON_BIN}" -c' in script,
        "Privacy Python SDK script must capture the selected Python version",
        errors,
    )
    require(
        '"${PYTHON_BIN}" --version' in script,
        "Privacy Python SDK script must print the selected Python version",
        errors,
    )
    require(
        'VENV_PYTHON_VERSION="$("${VENV_DIR}/bin/python" -c' in script,
        "Privacy Python SDK script must capture the venv Python version",
        errors,
    )
    require(
        script.count('"${VENV_DIR}/bin/python" --version') >= 2,
        "Privacy Python SDK script must print the initial and rebuilt venv Python versions",
        errors,
    )
    require(
        "3.11) ;;" in script,
        "Privacy Python SDK script must reject non-Python-3.11 runtimes",
        errors,
    )
    require(
        "recreating privacy Python SDK venv" in script,
        "Privacy Python SDK script must rebuild stale non-3.11 venvs",
        errors,
    )
    require(
        'rm -rf "${VENV_DIR}"' in script,
        "Privacy Python SDK script must remove stale venvs before rebuilding",
        errors,
    )
    require(
        "'maturin>=1.5,<2'" in script,
        "Privacy Python SDK script must install maturin for native extension builds",
        errors,
    )
    require(
        '"${VENV_DIR}/bin/python" -m pip install --no-deps' in script
        and '"${ROOT_DIR}/python/norito_py"' in script
        and '"${ROOT_DIR}/python/iroha_torii_client"' in script,
        "Privacy Python SDK script must install local workspace Python packages before maturin",
        errors,
    )
    require(
        'export VIRTUAL_ENV="${VENV_DIR}"' in script
        and 'export PATH="${VENV_DIR}/bin:${PATH}"' in script,
        "Privacy Python SDK script must activate the selected venv before maturin",
        errors,
    )
    require(
        '"${VENV_DIR}/bin/python" -m maturin develop --release' in script,
        "Privacy Python SDK script must build the native extension with the selected Python",
        errors,
    )
    require(
        "export PYTHONDONTWRITEBYTECODE=1" in script,
        "Privacy Python SDK script must not write bytecode cache files during tests",
        errors,
    )


def check_jvm_sdk_script_pins_jdk21(errors):
    script = read(jvm_sdk_command)
    require(
        'JAVA_HOME_OVERRIDE="${PRIVACY_JVM_SDK_JAVA_HOME:-}"' in script,
        "Privacy JVM SDK script must keep the documented Java home override variable",
        errors,
    )
    require(
        "PRIVACY_JVM_SDK_JAVA_HOME must point to a JDK 21 home." in script,
        "Privacy JVM SDK script must reject invalid explicit Java home overrides",
        errors,
    )
    require(
        "JAVA_HOME must point to a JDK 21 home for privacy JVM SDK tests." in script,
        "Privacy JVM SDK script must reject inherited non-JDK-21 JAVA_HOME values",
        errors,
    )
    require(
        "is_java_21_home()" in script,
        "Privacy JVM SDK script must validate Java homes as JDK 21",
        errors,
    )
    require(
        'version[[:space:]]+\\"21(\\.|\\")' in script,
        "Privacy JVM SDK script must match Java 21 version output",
        errors,
    )
    require(
        "java -version" in script,
        "Privacy JVM SDK script must print the selected Java version",
        errors,
    )


def run_checks():
    errors = []
    check_readmes(errors)
    check_zk_ace_proof_builder_coverage(errors)
    check_workflow_paths(errors)
    check_workflow_commands(errors)
    check_workflow_cancellation_policy(errors)
    check_workflow_runs_native_bridge_tests(errors)
    check_workflow_runs_swift_sdk_parse(errors)
    check_swift_sdk_script_prints_swiftc_version(errors)
    check_workflow_runs_jvm_sdk_tests(errors)
    check_workflow_runs_csharp_sdk_tests(errors)
    check_csharp_sdk_script_prints_dotnet_version(errors)
    check_workflow_runs_javascript_sdk_tests(errors)
    check_javascript_sdk_script_prints_node_version(errors)
    check_workflow_runs_python_sdk_tests(errors)
    check_jvm_sdk_script_pins_jdk21(errors)
    check_python_sdk_script_prints_python_version(errors)
    if errors:
        raise PrivacyGuardError("\n".join(errors))


if mode == "--negative-control-bridge-header-workflow-path":
    original = read(workflow_path)
    mutated = original.replace(
        '      - "ci/check_connect_norito_bridge_header.sh"\n',
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate bridge header path coverage")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy bridge header workflow path drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: bridge header workflow path drift was not detected")

if mode == "--negative-control-bridge-header-command-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        f"        run: {bridge_header_command}",
        "        run: ci/check_connect_norito_bridge_header.sh --skip",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate bridge header workflow command")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy bridge header workflow command drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: bridge header workflow command drift was not detected")

if mode == "--negative-control-bridge-header-negative-controls-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "ci/check_connect_norito_bridge_header.sh --negative-control-bad-privacy-signature",
        "ci/check_connect_norito_bridge_header.sh --synthetic-bad-privacy-signature-check",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate bridge header negative-control workflow command")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy bridge header negative-control workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: bridge header negative-control workflow drift was not detected")

if mode == "--negative-control-bytecode-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "      - name: Reject tracked Python bytecode\n"
        f"        run: {bytecode_command}\n",
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate tracked Python bytecode workflow command")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected tracked Python bytecode workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: tracked Python bytecode workflow drift was not detected")

if mode == "--negative-control-workflow-path":
    original = read(workflow_path)
    mutated = original.replace('      - "python/iroha_python/README.md"\n', "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate workflow path coverage")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy workflow path drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: workflow path drift was not detected")

if mode == "--negative-control-workflow-command":
    original = read(workflow_path)
    mutated = original.replace(
        "        run: ci/check_privacy_sdk_guard.sh",
        "        run: ci/check_privacy_sdk_guard.sh --skip-main-guard",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate workflow guard command")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy workflow command drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: workflow command drift was not detected")

if mode == "--negative-control-backend-tag-workflow-path":
    original = read(workflow_path)
    mutated = original.replace(
        '      - "IrohaSwift/Sources/IrohaSwift/VerifyingKeyBackendTag.swift"\n',
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate backend-tag workflow path coverage")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy backend-tag workflow path drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: backend-tag workflow path drift was not detected")

if mode == "--negative-control-browser-test-workflow-path":
    original = read(workflow_path)
    mutated = original.replace(
        '      - "javascript/iroha_js/test/crypto.browser.test.js"\n',
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate browser-test workflow path coverage")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy browser-test workflow path drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: browser-test workflow path drift was not detected")

if mode == "--negative-control-negative-controls-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "          ci/check_privacy_sdk_guard.sh --negative-control-readme-api",
        "          ci/check_privacy_sdk_guard.sh --synthetic-readme-api-check",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate workflow negative-control command")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy negative-control workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: negative-control workflow drift was not detected")

if mode == "--negative-control-negative-controls-comment-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "          ci/check_privacy_sdk_guard.sh --negative-control-readme-error-code",
        "          # ci/check_privacy_sdk_guard.sh --negative-control-readme-error-code",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to comment workflow negative-control command")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected commented privacy negative-control workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: commented negative-control workflow drift was not detected")

if mode == "--negative-control-negative-controls-order-workflow":
    original = read(workflow_path)
    command_line = "          ci/check_privacy_sdk_guard.sh --negative-control-readme-error-code\n"
    without_command = original.replace(command_line, "", 1)
    if without_command == original:
        raise SystemExit("negative control failed: unable to remove workflow negative-control command")
    mutated = without_command.replace(
        "        run: ci/check_privacy_sdk_guard.sh\n",
        "        run: ci/check_privacy_sdk_guard.sh\n" + command_line,
        1,
    )
    if mutated == without_command:
        raise SystemExit("negative control failed: unable to move workflow negative-control command")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected late privacy negative-control workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: late negative-control workflow drift was not detected")

if mode == "--negative-control-readme-boundary":
    target = "javascript/iroha_js/README.md"
    original = read(target)
    mutated = original.replace("privacy-production-gate-v1", "privacy-production-open-v1", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate README boundary")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy README boundary drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: README boundary drift was not detected")

if mode == "--negative-control-readme-api":
    target = "python/iroha_python/README.md"
    original = read(target)
    mutated = original.replace(
        "build_zk_ace_authorization_proof_v1()",
        "build_zk_ace_authorization_claim_v1()",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate README API name")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy README API drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: README API drift was not detected")

if mode == "--negative-control-zk-ace-proof-builder-coverage":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    mutated = original.replace(
        "  assertPythonZkAceProofBuilderCoverage();",
        "  assertPythonZkAceProofBuilderCoverageSkipped();",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate ZK-ACE proof-builder coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected ZK-ACE proof-builder coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: ZK-ACE proof-builder coverage drift was not detected")

if mode == "--negative-control-python-catalog-bytecode-guard":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    mutated = original.replace(
        '    env: { ...process.env, PYTHONDONTWRITEBYTECODE: "1" },\n',
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python catalog bytecode guard")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected Python catalog bytecode guard drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python catalog bytecode guard drift was not detected")

if mode == "--negative-control-readme-error-code":
    target = "javascript/iroha_js/README.md"
    original = read(target)
    mutated = original.replace("status_error = 1", "status_error = 0", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate README error-code value")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy README error-code drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: README error-code drift was not detected")

if mode == "--negative-control-browser-error-code":
    target = root / "javascript/iroha_js/src/crypto.browser.js"
    original = target.read_text(encoding="utf-8")
    mutated = original.replace(
        "export const PRIVACY_FFI_ERROR_INVALID_REQUEST = 5;",
        "export const PRIVACY_FFI_ERROR_INVALID_REQUEST = 6;",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate browser error-code value")
    result = None
    try:
        target.write_text(mutated, encoding="utf-8")
        result = subprocess.run(
            [
                "node",
                "--test",
                "--test-name-pattern",
                "browser crypto exposes native-only helpers as safe stubs",
                "test/crypto.browser.test.js",
            ],
            cwd=root / "javascript/iroha_js",
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            timeout=10,
            check=False,
        )
    finally:
        target.write_text(original, encoding="utf-8")
    output = result.stdout if result is not None else ""
    if result is not None and result.returncode != 0 and (
        "browser crypto exposes native-only helpers as safe stubs" in output
        or "Expected values to be strictly equal" in output
    ):
        print("negative control rejected browser privacy FFI error-code drift")
        first_line = next((line for line in output.splitlines() if line.strip()), "")
        if first_line:
            print(first_line)
        raise SystemExit(0)
    raise SystemExit("negative control failed: browser error-code drift was not detected")

if mode == "--negative-control-browser-dist-error-code":
    target = root / "javascript/iroha_js/dist/crypto.browser.js"
    original = target.read_text(encoding="utf-8")
    mutated = original.replace(
        "export const PRIVACY_FFI_ERROR_INVALID_REQUEST = 5;",
        "export const PRIVACY_FFI_ERROR_INVALID_REQUEST = 6;",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate browser dist error-code value")
    result = None
    try:
        target.write_text(mutated, encoding="utf-8")
        result = subprocess.run(
            [
                "node",
                "--test",
                "--test-name-pattern",
                "browser crypto exposes native-only helpers as safe stubs",
                "test/crypto.browser.test.js",
            ],
            cwd=root / "javascript/iroha_js",
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            timeout=10,
            check=False,
        )
    finally:
        target.write_text(original, encoding="utf-8")
    output = result.stdout if result is not None else ""
    if result is not None and result.returncode != 0 and (
        "browser crypto exposes native-only helpers as safe stubs" in output
        or "Expected values to be strictly equal" in output
    ):
        print("negative control rejected browser dist privacy FFI error-code drift")
        first_line = next((line for line in output.splitlines() if line.strip()), "")
        if first_line:
            print(first_line)
        raise SystemExit(0)
    raise SystemExit("negative control failed: browser dist error-code drift was not detected")

if mode == "--negative-control-workflow-cancel-in-progress":
    original = read(workflow_path)
    mutated = original.replace(
        "  cancel-in-progress: false",
        "  cancel-in-progress: true",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate workflow cancellation policy")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy workflow cancellation drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: workflow cancellation drift was not detected")

if mode == "--negative-control-native-bridge-job-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "  privacy_native_bridge_tests:\n",
        "  privacy_native_bridge_tests_disabled:\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate native bridge workflow job")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy native bridge workflow job drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: native bridge workflow job drift was not detected")

if mode == "--negative-control-native-bridge-runner-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "    runs-on: ubuntu-latest",
        "    runs-on: macos-latest",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate native bridge workflow runner")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy native bridge workflow runner drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: native bridge workflow runner drift was not detected")

if mode == "--negative-control-native-bridge-test-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        f"        run: {native_bridge_command}",
        "        run: cargo test -p connect_norito_bridge --lib -- --skip privacy_",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate native bridge test command")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy native bridge test workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: native bridge test workflow drift was not detected")

if mode == "--negative-control-native-bridge-needs-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        main_job_needs_line,
        "    needs: [privacy_swift_sdk_parse, privacy_jvm_sdk_tests, privacy_csharp_sdk_tests, privacy_javascript_sdk_tests, privacy_python_sdk_tests]",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate native bridge workflow dependency")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy native bridge workflow dependency drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: native bridge workflow dependency drift was not detected")

if mode == "--negative-control-swift-sdk-job-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "  privacy_swift_sdk_parse:\n",
        "  privacy_swift_sdk_parse_disabled:\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Swift SDK workflow job")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Swift SDK workflow job drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Swift SDK workflow job drift was not detected")

if mode == "--negative-control-swift-sdk-runner-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "    runs-on: macos-latest",
        "    runs-on: ubuntu-latest",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Swift SDK workflow runner")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Swift SDK workflow runner drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Swift SDK workflow runner drift was not detected")

if mode == "--negative-control-swift-sdk-parse-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        f"        run: {swift_sdk_command}",
        "        run: swiftc --version",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Swift SDK parse workflow command")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Swift SDK parse workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Swift SDK parse workflow drift was not detected")

if mode == "--negative-control-swift-sdk-version-script":
    original = read(swift_sdk_command)
    mutated = original.replace('"${SWIFTC_BIN}" --version\n', "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Swift SDK version evidence")
    text_overrides[swift_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Swift SDK version script drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Swift SDK version script drift was not detected")

if mode == "--negative-control-swift-sdk-override-script":
    original = read(swift_sdk_command)
    mutated = original.replace("PRIVACY_SWIFT_SDK_SWIFTC_BIN", "PRIVACY_SWIFT_SWIFTC_BIN", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Swift SDK compiler override variable")
    text_overrides[swift_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Swift SDK compiler override drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Swift SDK compiler override drift was not detected")

if mode == "--negative-control-swift-sdk-needs-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        main_job_needs_line,
        "    needs: [privacy_native_bridge_tests, privacy_jvm_sdk_tests, privacy_csharp_sdk_tests, privacy_javascript_sdk_tests, privacy_python_sdk_tests]",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Swift SDK workflow dependency")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Swift SDK workflow dependency drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Swift SDK workflow dependency drift was not detected")

if mode == "--negative-control-jvm-sdk-job-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "  privacy_jvm_sdk_tests:\n",
        "  privacy_jvm_sdk_tests_disabled:\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JVM SDK workflow job")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JVM SDK workflow job drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM SDK workflow job drift was not detected")

if mode == "--negative-control-jvm-sdk-setup-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "      - uses: actions/setup-java@v4\n",
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JVM SDK setup workflow step")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JVM SDK setup workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM SDK setup workflow drift was not detected")

if mode == "--negative-control-jvm-sdk-distribution-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        '          distribution: "temurin"\n',
        '          distribution: "zulu"\n',
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JVM SDK Java distribution")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JVM SDK Java distribution drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM SDK Java distribution drift was not detected")

if mode == "--negative-control-jvm-sdk-java-version-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        '          java-version: "21"\n',
        '          java-version: "17"\n',
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JVM SDK Java version")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JVM SDK Java version drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM SDK Java version drift was not detected")

if mode == "--negative-control-jvm-sdk-setup-order-workflow":
    original = read(workflow_path)
    run_line = f"        run: {jvm_sdk_command}\n"
    mutated = original.replace(run_line, "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JVM SDK setup order")
    insert = mutated.index("      - uses: actions/setup-java@v4\n")
    mutated = (
        mutated[:insert]
        + "      - name: Privacy JVM SDK tests\n"
        + run_line
        + mutated[insert:]
    )
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JVM SDK setup ordering drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM SDK setup ordering drift was not detected")

if mode == "--negative-control-jvm-sdk-test-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        f"        run: {jvm_sdk_command}",
        "        run: ci/check_privacy_jvm_sdk.sh --skip",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JVM SDK test workflow command")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JVM SDK test workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM SDK test workflow drift was not detected")

if mode == "--negative-control-jvm-sdk-jdk21-script":
    original = read(jvm_sdk_command)
    mutated = original.replace("java -version\n", "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JVM SDK JDK 21 script evidence")
    text_overrides[jvm_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JVM SDK JDK 21 script drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM SDK JDK 21 script drift was not detected")

if mode == "--negative-control-jvm-sdk-java-home-override-script":
    original = read(jvm_sdk_command)
    mutated = original.replace("PRIVACY_JVM_SDK_JAVA_HOME", "PRIVACY_JVM_SDK_JAVA_HOME_DISABLED", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JVM SDK Java home override variable")
    text_overrides[jvm_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JVM SDK Java home override drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM SDK Java home override drift was not detected")

if mode == "--negative-control-jvm-sdk-java-home-reject-script":
    original = read(jvm_sdk_command)
    mutated = original.replace(
        "JAVA_HOME must point to a JDK 21 home for privacy JVM SDK tests.",
        "JAVA_HOME is not checked before fallback.",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JVM SDK inherited Java home rejection")
    text_overrides[jvm_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JVM SDK inherited Java home rejection drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM SDK inherited Java home rejection drift was not detected")

if mode == "--negative-control-jvm-sdk-needs-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        main_job_needs_line,
        "    needs: [privacy_native_bridge_tests, privacy_swift_sdk_parse, privacy_csharp_sdk_tests, privacy_javascript_sdk_tests, privacy_python_sdk_tests]",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JVM SDK workflow dependency")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JVM SDK workflow dependency drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM SDK workflow dependency drift was not detected")

if mode == "--negative-control-csharp-sdk-job-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "  privacy_csharp_sdk_tests:\n",
        "  privacy_csharp_sdk_tests_disabled:\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate C# SDK workflow job")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy C# SDK workflow job drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# SDK workflow job drift was not detected")

if mode == "--negative-control-csharp-sdk-setup-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "      - uses: actions/setup-dotnet@v4\n",
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate C# SDK setup workflow step")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy C# SDK setup workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# SDK setup workflow drift was not detected")

if mode == "--negative-control-csharp-sdk-dotnet-version-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "          dotnet-version: 8.0.x\n",
        "          dotnet-version: 7.0.x\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate C# SDK dotnet version")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy C# SDK dotnet version drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# SDK dotnet version drift was not detected")

if mode == "--negative-control-csharp-sdk-setup-order-workflow":
    original = read(workflow_path)
    run_line = f"        run: {csharp_sdk_command}\n"
    mutated = original.replace(run_line, "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate C# SDK setup order")
    insert = mutated.index("      - uses: actions/setup-dotnet@v4\n")
    mutated = (
        mutated[:insert]
        + "      - name: Privacy C# SDK tests\n"
        + run_line
        + mutated[insert:]
    )
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy C# SDK setup ordering drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# SDK setup ordering drift was not detected")

if mode == "--negative-control-csharp-sdk-dotnet-version-script":
    original = read(csharp_sdk_command)
    mutated = original.replace('printf \'%s\\n\' "${DOTNET_VERSION}"\n', "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate C# SDK dotnet version evidence")
    text_overrides[csharp_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy C# SDK dotnet version script drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# SDK dotnet version script drift was not detected")

if mode == "--negative-control-csharp-sdk-dotnet-override-script":
    original = read(csharp_sdk_command)
    mutated = original.replace("PRIVACY_CSHARP_DOTNET_BIN", "PRIVACY_DOTNET_BIN", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate C# SDK dotnet override variable")
    text_overrides[csharp_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy C# SDK dotnet override drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# SDK dotnet override drift was not detected")

if mode == "--negative-control-csharp-sdk-dotnet-major-script":
    original = read(csharp_sdk_command)
    mutated = original.replace("8.0.*) ;;", "7.0.*) ;;", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate C# SDK dotnet major matcher")
    text_overrides[csharp_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy C# SDK dotnet major script drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# SDK dotnet major script drift was not detected")

if mode == "--negative-control-csharp-sdk-test-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        f"        run: {csharp_sdk_command}",
        "        run: ci/check_privacy_csharp_sdk.sh --skip",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate C# SDK test workflow command")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy C# SDK test workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# SDK test workflow drift was not detected")

if mode == "--negative-control-csharp-sdk-needs-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        main_job_needs_line,
        "    needs: [privacy_native_bridge_tests, privacy_swift_sdk_parse, privacy_jvm_sdk_tests, privacy_javascript_sdk_tests, privacy_python_sdk_tests]",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate C# SDK workflow dependency")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy C# SDK workflow dependency drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# SDK workflow dependency drift was not detected")

if mode == "--negative-control-js-sdk-job-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "  privacy_javascript_sdk_tests:\n",
        "  privacy_javascript_sdk_tests_disabled:\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK workflow job")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JavaScript SDK workflow job drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK workflow job drift was not detected")

if mode == "--negative-control-js-sdk-runner-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "  privacy_javascript_sdk_tests:\n    runs-on: ubuntu-latest",
        "  privacy_javascript_sdk_tests:\n    runs-on: macos-latest",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK workflow runner")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JavaScript SDK workflow runner drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK workflow runner drift was not detected")

if mode == "--negative-control-js-sdk-node-setup-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "      - uses: actions/setup-node@v4\n",
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK Node setup workflow step")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JavaScript SDK Node setup drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK Node setup drift was not detected")

if mode == "--negative-control-js-sdk-node-version-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        '          node-version: "20"\n',
        '          node-version: "18"\n',
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK Node version")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JavaScript SDK Node version drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK Node version drift was not detected")

if mode == "--negative-control-js-sdk-node-version-script":
    original = read(js_sdk_command)
    mutated = original.replace('printf \'%s\\n\' "${NODE_VERSION}"\n', "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK Node version evidence")
    text_overrides[js_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JavaScript SDK Node version script drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK Node version script drift was not detected")

if mode == "--negative-control-js-sdk-node-override-script":
    original = read(js_sdk_command)
    mutated = original.replace("PRIVACY_JS_SDK_NODE_BIN", "PRIVACY_JS_NODE_BIN", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK Node override variable")
    text_overrides[js_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JavaScript SDK Node override drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK Node override drift was not detected")

if mode == "--negative-control-js-sdk-node-resolver-script":
    original = read(js_sdk_command)
    mutated = original.replace("resolve_node_20_bin()", "resolve_node_bin()", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK Node resolver")
    text_overrides[js_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JavaScript SDK Node resolver drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK Node resolver drift was not detected")

if mode == "--negative-control-js-sdk-node-major-script":
    original = read(js_sdk_command)
    mutated = original.replace("v20.*) ;;", "v18.*) ;;", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK Node major matcher")
    text_overrides[js_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JavaScript SDK Node major script drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK Node major script drift was not detected")

if mode == "--negative-control-js-sdk-node-cache-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "          cache-dependency-path: javascript/iroha_js/package-lock.json\n",
        "          cache-dependency-path: javascript/iroha_js/package.json\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK cache path")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JavaScript SDK cache path drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK cache path drift was not detected")

if mode == "--negative-control-js-sdk-node-setup-order-workflow":
    original = read(workflow_path)
    install_block = (
        "      - name: Install JavaScript SDK dependencies\n"
        f"        run: {js_sdk_install_command}\n"
    )
    mutated = original.replace(install_block, "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to move JavaScript SDK install before Node setup")
    insert = mutated.index("      - uses: actions/setup-node@v4\n")
    mutated = mutated[:insert] + install_block + mutated[insert:]
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JavaScript SDK Node setup ordering drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK Node setup ordering drift was not detected")

if mode == "--negative-control-js-sdk-install-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        f"        run: {js_sdk_install_command}",
        "        run: npm ci --prefix javascript/iroha_js --ignore-scripts",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK install workflow command")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JavaScript SDK install workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK install workflow drift was not detected")

if mode == "--negative-control-js-sdk-test-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        f"        run: {js_sdk_command}",
        "        run: ci/check_privacy_js_sdk.sh --skip",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK test workflow command")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JavaScript SDK test workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK test workflow drift was not detected")

if mode == "--negative-control-js-sdk-install-order-workflow":
    original = read(workflow_path)
    install_line = f"        run: {js_sdk_install_command}"
    test_line = f"        run: {js_sdk_command}"
    mutated = original.replace(f"{install_line}\n", "", 1)
    mutated = mutated.replace(f"{test_line}\n", f"{test_line}\n{install_line}\n", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to move JavaScript SDK install after tests")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JavaScript SDK install ordering drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK install ordering drift was not detected")

if mode == "--negative-control-js-sdk-test-order-workflow":
    original = read(workflow_path)
    test_line = f"        run: {js_sdk_command}"
    mutated = original.replace(f"{test_line}\n", "", 1)
    mutated = mutated.replace(
        "        run: ci/check_privacy_sdk_guard.sh\n",
        "        run: ci/check_privacy_sdk_guard.sh\n" + test_line + "\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to move JavaScript SDK tests after main guard")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JavaScript SDK test ordering drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK test ordering drift was not detected")

if mode == "--negative-control-js-sdk-needs-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        main_job_needs_line,
        "    needs: [privacy_native_bridge_tests, privacy_swift_sdk_parse, privacy_jvm_sdk_tests, privacy_csharp_sdk_tests, privacy_python_sdk_tests]",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK workflow dependency")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JavaScript SDK workflow dependency drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK workflow dependency drift was not detected")

if mode == "--negative-control-python-sdk-job-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "  privacy_python_sdk_tests:\n",
        "  privacy_python_sdk_tests_disabled:\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK workflow job")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Python SDK workflow job drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK workflow job drift was not detected")

if mode == "--negative-control-python-sdk-runner-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "  privacy_python_sdk_tests:\n    runs-on: ubuntu-latest",
        "  privacy_python_sdk_tests:\n    runs-on: macos-latest",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK workflow runner")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Python SDK workflow runner drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK workflow runner drift was not detected")

if mode == "--negative-control-python-sdk-setup-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "      - uses: actions/setup-python@v5\n",
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK setup workflow step")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Python SDK setup workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK setup workflow drift was not detected")

if mode == "--negative-control-python-sdk-version-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        '          python-version: "3.11"\n',
        '          python-version: "3.10"\n',
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK version")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Python SDK version drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK version drift was not detected")

if mode == "--negative-control-python-sdk-setup-order-workflow":
    original = read(workflow_path)
    run_line = f"        run: {python_sdk_command}\n"
    mutated = original.replace(run_line, "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK setup order")
    insert = mutated.index("      - uses: actions/setup-python@v5\n")
    mutated = (
        mutated[:insert]
        + "      - name: Privacy Python SDK tests\n"
        + run_line
        + mutated[insert:]
    )
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Python SDK setup ordering drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK setup ordering drift was not detected")

if mode == "--negative-control-python-sdk-rust-cache-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "  privacy_python_sdk_tests:\n"
        "    runs-on: ubuntu-latest\n"
        "    timeout-minutes: 45\n"
        "    steps:\n"
        "      - uses: actions/checkout@v4\n"
        "      - uses: Swatinem/rust-cache@v2\n"
        "        with:\n"
        "          cache-on-failure: \"true\"\n",
        "  privacy_python_sdk_tests:\n"
        "    runs-on: ubuntu-latest\n"
        "    timeout-minutes: 45\n"
        "    steps:\n"
        "      - uses: actions/checkout@v4\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK Rust cache")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Python SDK Rust cache drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK Rust cache drift was not detected")

if mode == "--negative-control-python-sdk-timeout-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "  privacy_python_sdk_tests:\n"
        "    runs-on: ubuntu-latest\n"
        "    timeout-minutes: 45\n",
        "  privacy_python_sdk_tests:\n"
        "    runs-on: ubuntu-latest\n"
        "    timeout-minutes: 15\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK timeout")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Python SDK timeout drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK timeout drift was not detected")

if mode == "--negative-control-python-sdk-version-script":
    original = read(python_sdk_command)
    mutated = original.replace('"${VENV_DIR}/bin/python" --version\n', "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK version evidence")
    text_overrides[python_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Python SDK version script drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK version script drift was not detected")

if mode == "--negative-control-python-sdk-override-script":
    original = read(python_sdk_command)
    mutated = original.replace("PRIVACY_PYTHON_SDK_PYTHON_BIN", "PRIVACY_PYTHON_BIN", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK override variable")
    text_overrides[python_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Python SDK override drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK override drift was not detected")

if mode == "--negative-control-python-sdk-resolver-script":
    original = read(python_sdk_command)
    mutated = original.replace("resolve_python_311_bin()", "resolve_python_bin()", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK resolver")
    text_overrides[python_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Python SDK resolver drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK resolver drift was not detected")

if mode == "--negative-control-python-sdk-major-script":
    original = read(python_sdk_command)
    mutated = original.replace("3.11) ;;", "3.10) ;;")
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK major matcher")
    text_overrides[python_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Python SDK major script drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK major script drift was not detected")

if mode == "--negative-control-python-sdk-venv-rebuild-script":
    original = read(python_sdk_command)
    mutated = original.replace('  rm -rf "${VENV_DIR}"\n', "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK stale venv rebuild")
    text_overrides[python_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Python SDK stale venv rebuild drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK stale venv rebuild drift was not detected")

if mode == "--negative-control-python-sdk-native-build-script":
    original = read(python_sdk_command)
    mutated = original.replace('"${VENV_DIR}/bin/python" -m maturin develop --release\n', "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK native build step")
    text_overrides[python_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Python SDK native build script drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK native build script drift was not detected")

if mode == "--negative-control-python-sdk-venv-activation-script":
    original = read(python_sdk_command)
    mutated = original.replace('export VIRTUAL_ENV="${VENV_DIR}"\nexport PATH="${VENV_DIR}/bin:${PATH}"\n\n', "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK venv activation")
    text_overrides[python_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Python SDK venv activation drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK venv activation drift was not detected")

if mode == "--negative-control-python-sdk-bytecode-script":
    original = read(python_sdk_command)
    mutated = original.replace("export PYTHONDONTWRITEBYTECODE=1\n\n", "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK bytecode guard")
    text_overrides[python_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Python SDK bytecode script drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK bytecode script drift was not detected")

if mode == "--negative-control-python-sdk-test-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        f"        run: {python_sdk_command}",
        "        run: ci/check_privacy_python_sdk.sh --skip",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK test workflow command")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Python SDK test workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK test workflow drift was not detected")

if mode == "--negative-control-python-sdk-needs-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        main_job_needs_line,
        "    needs: [privacy_native_bridge_tests, privacy_swift_sdk_parse, privacy_jvm_sdk_tests, privacy_csharp_sdk_tests, privacy_javascript_sdk_tests]",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK workflow dependency")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Python SDK workflow dependency drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK workflow dependency drift was not detected")

if mode == "--negative-control-main-rust-cache-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "  privacy-sdk-guard:\n"
        f"{main_job_needs_line}\n"
        "    runs-on: ubuntu-latest\n"
        "    timeout-minutes: 45\n"
        "    steps:\n"
        "      - uses: actions/checkout@v4\n"
        "      - uses: Swatinem/rust-cache@v2\n"
        "        with:\n"
        "          cache-on-failure: \"true\"\n",
        "  privacy-sdk-guard:\n"
        f"{main_job_needs_line}\n"
        "    runs-on: ubuntu-latest\n"
        "    timeout-minutes: 45\n"
        "    steps:\n"
        "      - uses: actions/checkout@v4\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate main guard Rust cache")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy main guard Rust cache drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: main guard Rust cache drift was not detected")

if mode == "--negative-control-main-timeout-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "  privacy-sdk-guard:\n"
        f"{main_job_needs_line}\n"
        "    runs-on: ubuntu-latest\n"
        "    timeout-minutes: 45\n",
        "  privacy-sdk-guard:\n"
        f"{main_job_needs_line}\n"
        "    runs-on: ubuntu-latest\n"
        "    timeout-minutes: 20\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate main guard timeout")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy main guard timeout drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: main guard timeout drift was not detected")

if mode:
    raise SystemExit(f"unknown mode: {mode}")

run_checks()
PY

if [[ -n "${MODE}" ]]; then
  exit 0
fi

SDK_NODE_BIN="$(resolve_node_20_bin)"
SDK_PYTHON_BIN="$(resolve_python_311_bin)"

PRIVACY_JS_SDK_ROOT="${ROOT_DIR}" \
  PRIVACY_JS_SDK_NODE_BIN="${SDK_NODE_BIN}" \
  bash "${ROOT_DIR}/ci/check_privacy_js_sdk.sh"
PRIVACY_PYTHON_SDK_ROOT="${ROOT_DIR}" \
  PRIVACY_PYTHON_SDK_PYTHON_BIN="${SDK_PYTHON_BIN}" \
  PRIVACY_PYTHON_SDK_VENV="${VENV_DIR}" \
  bash "${ROOT_DIR}/ci/check_privacy_python_sdk.sh"
