#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${SORAFS_PIN_REGISTER_SDK_GUARD_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
MODE="${1:-}"

python3 - "${ROOT_DIR}" "${MODE}" <<'PY'
import re
import sys
from pathlib import Path

root = Path(sys.argv[1])
mode = sys.argv[2]
text_overrides = {}

workflow_path = ".github/workflows/pr_sorafs_pin_register_sdk.yml"
main_job = "sorafs-pin-register-sdk-guard"
swift_job = "sorafs_pin_register_swift_sdk_check"
jvm_job = "sorafs_pin_register_jvm_sdk_tests"
csharp_job = "sorafs_pin_register_csharp_sdk_tests"
js_job = "sorafs_pin_register_javascript_sdk_tests"
python_job = "sorafs_pin_register_python_sdk_tests"

main_command = "bash ci/check_sorafs_pin_register_sdk_guard.sh"
bytecode_command = "bash ci/check_no_tracked_python_bytecode.sh"
swift_command = "bash ci/check_sorafs_pin_register_swift_sdk.sh"
jvm_command = "bash ci/check_sorafs_pin_register_jvm_sdk.sh"
csharp_command = "bash ci/check_sorafs_pin_register_csharp_sdk.sh"
js_install_command = "npm ci --prefix javascript/iroha_js"
js_command = "bash ci/check_sorafs_pin_register_js_sdk.sh"
python_command = "bash ci/check_sorafs_pin_register_python_sdk.sh"
main_job_needs_line = (
    "    needs: [sorafs_pin_register_swift_sdk_check, "
    "sorafs_pin_register_jvm_sdk_tests, sorafs_pin_register_csharp_sdk_tests, "
    "sorafs_pin_register_javascript_sdk_tests, sorafs_pin_register_python_sdk_tests]"
)

required_paths = (
    workflow_path,
    "ci/check_sorafs_pin_register_csharp_sdk.sh",
    "ci/check_no_tracked_python_bytecode.sh",
    "ci/check_sorafs_pin_register_js_sdk.sh",
    "ci/check_sorafs_pin_register_jvm_sdk.sh",
    "ci/check_sorafs_pin_register_python_sdk.sh",
    "ci/check_sorafs_pin_register_sdk_guard.sh",
    "ci/check_sorafs_pin_register_swift_sdk.sh",
    "IrohaSwift/Sources/IrohaSwift/ToriiClient.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift",
    "csharp/src/Hyperledger.Iroha.Sdk/Torii/ToriiClient.cs",
    "csharp/src/Hyperledger.Iroha.Sdk/Torii/ToriiModels.cs",
    "csharp/src/Hyperledger.Iroha.Sdk/Torii/ToriiJsonSerializerContext.cs",
    "csharp/tests/Hyperledger.Iroha.Sdk.Tests/ToriiClientTests.cs",
    "csharp/tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/instructions/RegisterPinManifestInstruction.java",
    "java/iroha_android/src/test/java/org/hyperledger/iroha/android/sorafs/SorafsRegisterPinManifestBuilderTests.java",
    "java/iroha_android/src/test/java/org/hyperledger/iroha/android/testing/SimpleJson.java",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/core/model/instructions/RegisterPinManifestInstruction.kt",
    "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/core/model/instructions/RegisterPinManifestInstructionTest.kt",
    "javascript/iroha_js/package.json",
    "javascript/iroha_js/package-lock.json",
    "javascript/iroha_js/src/toriiClient.js",
    "javascript/iroha_js/dist/toriiClient.js",
    "javascript/iroha_js/index.d.ts",
    "javascript/iroha_js/test/toriiClient.test.js",
    "javascript/iroha_js/test/sorafsPinRegisterSdkGuard.test.js",
    "python/iroha_python/src/iroha_python/__init__.py",
    "python/iroha_python/src/iroha_python/client.py",
    "python/iroha_python/tests/client_sorafs_pin_register_test.py",
    "roadmap.md",
    "status.md",
)

negative_control_commands = (
    ("workflow path negative control", f"{main_command} --negative-control-workflow-path"),
    ("workflow command negative control", f"{main_command} --negative-control-workflow-command"),
    (
        "negative-control workflow negative control",
        f"{main_command} --negative-control-negative-controls-workflow",
    ),
    (
        "commented negative-control workflow negative control",
        f"{main_command} --negative-control-negative-controls-comment-workflow",
    ),
    (
        "negative-control ordering workflow negative control",
        f"{main_command} --negative-control-negative-controls-order-workflow",
    ),
    (
        "tracked Python bytecode workflow negative control",
        f"{main_command} --negative-control-bytecode-workflow",
    ),
    ("Swift SDK job workflow negative control", f"{main_command} --negative-control-swift-sdk-job-workflow"),
    ("Swift SDK runner workflow negative control", f"{main_command} --negative-control-swift-sdk-runner-workflow"),
    ("Swift SDK script workflow negative control", f"{main_command} --negative-control-swift-sdk-script-workflow"),
    ("Swift SDK version script negative control", f"{main_command} --negative-control-swift-sdk-version-script"),
    ("Swift SDK compiler override script negative control", f"{main_command} --negative-control-swift-sdk-override-script"),
    ("Swift SDK dependency workflow negative control", f"{main_command} --negative-control-swift-sdk-needs-workflow"),
    ("JVM SDK job workflow negative control", f"{main_command} --negative-control-jvm-sdk-job-workflow"),
    ("JVM SDK setup workflow negative control", f"{main_command} --negative-control-jvm-sdk-setup-workflow"),
    ("JVM SDK Java distribution workflow negative control", f"{main_command} --negative-control-jvm-sdk-distribution-workflow"),
    ("JVM SDK Java version workflow negative control", f"{main_command} --negative-control-jvm-sdk-java-version-workflow"),
    ("JVM SDK Java setup ordering workflow negative control", f"{main_command} --negative-control-jvm-sdk-setup-order-workflow"),
    ("JVM SDK script workflow negative control", f"{main_command} --negative-control-jvm-sdk-script-workflow"),
    ("JVM SDK JDK 21 script negative control", f"{main_command} --negative-control-jvm-sdk-jdk21-script"),
    ("JVM SDK Java home override script negative control", f"{main_command} --negative-control-jvm-sdk-java-home-override-script"),
    ("JVM SDK inherited Java home rejection script negative control", f"{main_command} --negative-control-jvm-sdk-java-home-reject-script"),
    ("JVM SDK dependency workflow negative control", f"{main_command} --negative-control-jvm-sdk-needs-workflow"),
    ("C# SDK job workflow negative control", f"{main_command} --negative-control-csharp-sdk-job-workflow"),
    ("C# SDK setup workflow negative control", f"{main_command} --negative-control-csharp-sdk-setup-workflow"),
    ("C# SDK dotnet version workflow negative control", f"{main_command} --negative-control-csharp-sdk-dotnet-version-workflow"),
    ("C# SDK setup ordering workflow negative control", f"{main_command} --negative-control-csharp-sdk-setup-order-workflow"),
    ("C# SDK dotnet version script negative control", f"{main_command} --negative-control-csharp-sdk-dotnet-version-script"),
    ("C# SDK dotnet override script negative control", f"{main_command} --negative-control-csharp-sdk-dotnet-override-script"),
    ("C# SDK dotnet major script negative control", f"{main_command} --negative-control-csharp-sdk-dotnet-major-script"),
    ("C# SDK script workflow negative control", f"{main_command} --negative-control-csharp-sdk-script-workflow"),
    ("C# SDK dependency workflow negative control", f"{main_command} --negative-control-csharp-sdk-needs-workflow"),
    ("JavaScript SDK job workflow negative control", f"{main_command} --negative-control-js-sdk-job-workflow"),
    ("JavaScript SDK runner workflow negative control", f"{main_command} --negative-control-js-sdk-runner-workflow"),
    ("JavaScript SDK Node setup workflow negative control", f"{main_command} --negative-control-js-sdk-node-setup-workflow"),
    ("JavaScript SDK Node version workflow negative control", f"{main_command} --negative-control-js-sdk-node-version-workflow"),
    ("JavaScript SDK Node version script negative control", f"{main_command} --negative-control-js-sdk-node-version-script"),
    ("JavaScript SDK Node override script negative control", f"{main_command} --negative-control-js-sdk-node-override-script"),
    ("JavaScript SDK Node resolver script negative control", f"{main_command} --negative-control-js-sdk-node-resolver-script"),
    ("JavaScript SDK Node major script negative control", f"{main_command} --negative-control-js-sdk-node-major-script"),
    ("JavaScript SDK Node cache workflow negative control", f"{main_command} --negative-control-js-sdk-node-cache-workflow"),
    ("JavaScript SDK Node setup ordering workflow negative control", f"{main_command} --negative-control-js-sdk-node-setup-order-workflow"),
    ("JavaScript SDK install workflow negative control", f"{main_command} --negative-control-js-sdk-install-workflow"),
    ("JavaScript SDK script workflow negative control", f"{main_command} --negative-control-js-sdk-script-workflow"),
    ("JavaScript SDK install ordering workflow negative control", f"{main_command} --negative-control-js-sdk-install-order-workflow"),
    ("JavaScript SDK test ordering workflow negative control", f"{main_command} --negative-control-js-sdk-test-order-workflow"),
    ("JavaScript SDK dependency workflow negative control", f"{main_command} --negative-control-js-sdk-needs-workflow"),
    ("Python SDK job workflow negative control", f"{main_command} --negative-control-python-sdk-job-workflow"),
    ("Python SDK setup workflow negative control", f"{main_command} --negative-control-python-sdk-setup-workflow"),
    ("Python SDK version workflow negative control", f"{main_command} --negative-control-python-sdk-version-workflow"),
    ("Python SDK setup ordering workflow negative control", f"{main_command} --negative-control-python-sdk-setup-order-workflow"),
    ("Python SDK version script negative control", f"{main_command} --negative-control-python-sdk-version-script"),
    ("Python SDK override script negative control", f"{main_command} --negative-control-python-sdk-override-script"),
    ("Python SDK resolver script negative control", f"{main_command} --negative-control-python-sdk-resolver-script"),
    ("Python SDK major script negative control", f"{main_command} --negative-control-python-sdk-major-script"),
    ("Python SDK stale venv rebuild script negative control", f"{main_command} --negative-control-python-sdk-venv-rebuild-script"),
    ("Python SDK bytecode script negative control", f"{main_command} --negative-control-python-sdk-bytecode-script"),
    ("Python SDK script workflow negative control", f"{main_command} --negative-control-python-sdk-script-workflow"),
    ("Python SDK dependency workflow negative control", f"{main_command} --negative-control-python-sdk-needs-workflow"),
    ("JavaScript source negative control", f"{main_command} --negative-control-js-source-endpoint"),
    ("JavaScript adversarial test negative control", f"{main_command} --negative-control-js-adversarial-test"),
    ("Python adversarial test negative control", f"{main_command} --negative-control-python-adversarial-test"),
    ("Swift contract test negative control", f"{main_command} --negative-control-swift-contract-test"),
    ("C# malformed response test negative control", f"{main_command} --negative-control-csharp-malformed-response-test"),
    ("Kotlin builder test negative control", f"{main_command} --negative-control-kotlin-builder-test"),
    ("Kotlin chunker unsigned test negative control", f"{main_command} --negative-control-kotlin-chunker-unsigned-test"),
    ("Java builder test negative control", f"{main_command} --negative-control-java-builder-test"),
    ("Java chunker unsigned test negative control", f"{main_command} --negative-control-java-chunker-unsigned-test"),
)


class GuardError(Exception):
    pass


def read(path):
    if path in text_overrides:
        return text_overrides[path]
    full_path = root / path
    try:
        return full_path.read_text(encoding="utf-8")
    except FileNotFoundError as exc:
        raise GuardError(f"required file is missing: {path}") from exc


def require(condition, message):
    if not condition:
        raise GuardError(message)


def require_contains(path, needle, label):
    require(needle in read(path), f"{path} missing {label}: {needle}")


def require_min_count(path, needle, minimum, label):
    actual = read(path).count(needle)
    require(
        actual >= minimum,
        f"{path} missing {label}: expected at least {minimum}, got {actual}: {needle}",
    )


def workflow_job(workflow, job):
    marker = f"  {job}:\n"
    start = workflow.find(marker)
    require(start >= 0, f"workflow missing job: {job}")
    next_match = re.search(r"\n  [A-Za-z0-9_-]+:\n", workflow[start + len(marker) :])
    if next_match is None:
        return workflow[start:]
    return workflow[start : start + len(marker) + next_match.start()]


def require_workflow_path(workflow, path):
    require(f'      - "{path}"' in workflow, f"workflow pull_request paths must include {path}")


def require_run(job_text, command, label):
    require(f"        run: {command}\n" in job_text, f"{label} must run `{command}`")


def require_negative_controls(workflow):
    for label, command in negative_control_commands:
        require(
            f"          {command}\n" in workflow,
            f"workflow must execute {label}: {command}",
        )
    first_negative = workflow.index(f"          {negative_control_commands[0][1]}\n")
    guard_command = workflow.index(f"        run: {main_command}")
    require(first_negative < guard_command, "negative controls must run before the main guard")


def check_workflow():
    workflow = read(workflow_path)
    require("name: SoraFS Pin Register SDK Guard" in workflow, "workflow has unexpected name")
    require("cancel-in-progress: false" in workflow, "workflow must not cancel in-progress checks")
    for path in required_paths:
        require_workflow_path(workflow, path)

    swift = workflow_job(workflow, swift_job)
    require("    runs-on: macos-latest" in swift, "Swift SDK check must run on macOS")
    require_run(swift, swift_command, "Swift SDK check")

    jvm = workflow_job(workflow, jvm_job)
    require("    runs-on: ubuntu-latest" in jvm, "JVM SDK tests must run on Ubuntu")
    java_setup_match = re.search(r"(?m)^\s+- uses:\s+actions/setup-java@v4\s*$", jvm)
    java_command_match = re.search(rf"(?m)^\s+run:\s+{re.escape(jvm_command)}\s*$", jvm)
    require(java_setup_match is not None, "JVM SDK tests must set up Java")
    require(re.search(r'(?m)^\s+distribution:\s+"temurin"\s*$', jvm) is not None, "JVM SDK tests must pin Temurin Java")
    require(re.search(r'(?m)^\s+java-version:\s+"21"\s*$', jvm) is not None, "JVM SDK tests must pin Java 21")
    require_run(jvm, jvm_command, "JVM SDK tests")
    if java_setup_match is not None and java_command_match is not None:
        require(
            java_setup_match.start() < java_command_match.start(),
            "JVM SDK tests must set up Java before running tests",
        )

    csharp = workflow_job(workflow, csharp_job)
    require("    runs-on: ubuntu-latest" in csharp, "C# SDK tests must run on Ubuntu")
    dotnet_setup_match = re.search(r"(?m)^\s+- uses:\s+actions/setup-dotnet@v4\s*$", csharp)
    dotnet_command_match = re.search(rf"(?m)^\s+run:\s+{re.escape(csharp_command)}\s*$", csharp)
    require(dotnet_setup_match is not None, "C# SDK tests must set up .NET")
    require(re.search(r"(?m)^\s+dotnet-version:\s+8\.0\.x\s*$", csharp) is not None, "C# SDK tests must pin .NET 8")
    require_run(csharp, csharp_command, "C# SDK tests")
    if dotnet_setup_match is not None and dotnet_command_match is not None:
        require(
            dotnet_setup_match.start() < dotnet_command_match.start(),
            "C# SDK tests must set up .NET before running tests",
        )

    js = workflow_job(workflow, js_job)
    require("    runs-on: ubuntu-latest" in js, "JavaScript SDK tests must run on Ubuntu")
    node_setup_match = re.search(r"(?m)^\s+- uses:\s+actions/setup-node@v4\s*$", js)
    node_install_match = re.search(rf"(?m)^\s+run:\s+{re.escape(js_install_command)}\s*$", js)
    require(node_setup_match is not None, "JavaScript SDK tests must set up Node")
    require(re.search(r'(?m)^\s+node-version:\s+"20"\s*$', js) is not None, "JavaScript SDK tests must pin Node 20")
    require(
        re.search(
            r"(?m)^\s+cache-dependency-path:\s+javascript/iroha_js/package-lock\.json\s*$",
            js,
        )
        is not None,
        "JavaScript SDK tests must cache dependencies by package-lock",
    )
    require(f"        run: {js_install_command}" in js, "JavaScript SDK tests must install dependencies")
    require_run(js, js_command, "JavaScript SDK tests")
    if node_setup_match is not None and node_install_match is not None:
        require(
            node_setup_match.start() < node_install_match.start(),
            "JavaScript SDK tests must set up Node before installing dependencies",
        )
    require(
        js.index(f"        run: {js_install_command}") < js.index(f"        run: {js_command}"),
        "JavaScript SDK dependency install must run before tests",
    )

    python = workflow_job(workflow, python_job)
    require("    runs-on: ubuntu-latest" in python, "Python SDK tests must run on Ubuntu")
    python_setup_match = re.search(r"(?m)^\s+- uses:\s+actions/setup-python@v5\s*$", python)
    python_command_match = re.search(rf"(?m)^\s+run:\s+{re.escape(python_command)}\s*$", python)
    require(python_setup_match is not None, "Python SDK tests must set up Python")
    require(re.search(r'(?m)^\s+python-version:\s+"3\.11"\s*$', python) is not None, "Python SDK tests must pin Python 3.11")
    require_run(python, python_command, "Python SDK tests")
    if python_setup_match is not None and python_command_match is not None:
        require(
            python_setup_match.start() < python_command_match.start(),
            "Python SDK tests must set up Python before running tests",
        )

    main = workflow_job(workflow, main_job)
    require(main_job_needs_line in main, "main SoraFS guard must depend on every SDK lane")
    require_run(main, bytecode_command, "tracked Python bytecode guard")
    require_run(main, main_command, "main SoraFS SDK guard")
    require(
        main.index(f"        run: {bytecode_command}\n") < main.index(f"        run: {main_command}\n"),
        "tracked Python bytecode guard must run before the main SoraFS SDK guard",
    )
    require_negative_controls(workflow)


def check_scripts():
    require_contains("ci/check_sorafs_pin_register_js_sdk.sh", 'registerSorafsPinManifest|SoraFS pin-register SDK guard|SoraFS .* SDK runner', "focused JS test pattern")
    require_contains("ci/check_sorafs_pin_register_js_sdk.sh", 'NODE_OVERRIDE="${SORAFS_PIN_REGISTER_JS_SDK_NODE_BIN:-}"', "JavaScript SDK Node override variable")
    require_contains("ci/check_sorafs_pin_register_js_sdk.sh", "resolve_node_20_bin()", "JavaScript SDK Node 20 resolver")
    require_contains("ci/check_sorafs_pin_register_js_sdk.sh", "is_node_20_bin()", "JavaScript SDK Node 20 version predicate")
    require_contains("ci/check_sorafs_pin_register_js_sdk.sh", 'NODE_BIN="$(resolve_node_20_bin)"', "JavaScript SDK selected Node resolver")
    require_contains("ci/check_sorafs_pin_register_js_sdk.sh", 'NODE_VERSION="$("${NODE_BIN}" --version)"', "JavaScript SDK selected Node capture")
    require_contains("ci/check_sorafs_pin_register_js_sdk.sh", 'printf \'%s\\n\' "${NODE_VERSION}"', "JavaScript SDK Node version evidence")
    require_contains("ci/check_sorafs_pin_register_js_sdk.sh", "v20.*) ;;", "JavaScript SDK Node 20 matcher")
    require_contains("ci/check_sorafs_pin_register_python_sdk.sh", "client_sorafs_pin_register_test.py", "Python paid-pin test suite")
    require_contains("ci/check_sorafs_pin_register_python_sdk.sh", 'PYTHON_OVERRIDE="${SORAFS_PIN_REGISTER_PYTHON_BIN:-}"', "Python SDK override variable")
    require_contains("ci/check_sorafs_pin_register_python_sdk.sh", "resolve_python_311_bin()", "Python SDK 3.11 resolver")
    require_contains("ci/check_sorafs_pin_register_python_sdk.sh", "python3.11", "Python SDK 3.11 resolver candidate")
    require_contains("ci/check_sorafs_pin_register_python_sdk.sh", 'PYTHON_BIN="$(resolve_python_311_bin)"', "Python SDK selected resolver")
    require_contains("ci/check_sorafs_pin_register_python_sdk.sh", 'PYTHON_VERSION="$("${PYTHON_BIN}" -c', "Python SDK selected version capture")
    require_contains("ci/check_sorafs_pin_register_python_sdk.sh", '"${PYTHON_BIN}" --version', "Python SDK selected version evidence")
    require_contains("ci/check_sorafs_pin_register_python_sdk.sh", 'VENV_PYTHON_VERSION="$("${VENV_DIR}/bin/python" -c', "Python SDK venv version capture")
    require_min_count(
        "ci/check_sorafs_pin_register_python_sdk.sh",
        '"${VENV_DIR}/bin/python" --version',
        2,
        "Python SDK initial and rebuilt venv version evidence",
    )
    require_contains("ci/check_sorafs_pin_register_python_sdk.sh", "3.11) ;;", "Python SDK 3.11 matcher")
    require_contains("ci/check_sorafs_pin_register_python_sdk.sh", "recreating SoraFS pin-register Python SDK venv", "Python SDK stale venv rebuild evidence")
    require_contains("ci/check_sorafs_pin_register_python_sdk.sh", 'rm -rf "${VENV_DIR}"', "Python SDK stale venv removal")
    require_contains("ci/check_sorafs_pin_register_python_sdk.sh", "export PYTHONDONTWRITEBYTECODE=1", "Python SDK bytecode-cache guard")
    require_contains("ci/check_sorafs_pin_register_python_sdk.sh", '"${ROOT_DIR}/python/norito_py"', "Python SDK local Norito package install")
    require_contains("ci/check_sorafs_pin_register_python_sdk.sh", '"${ROOT_DIR}/python/iroha_torii_client"', "Python SDK local Torii package install")
    require_contains("ci/check_sorafs_pin_register_python_sdk.sh", 'PYTHONPATH="${ROOT_DIR}/python/iroha_python/src:${ROOT_DIR}/python/norito_py/src:${ROOT_DIR}/python"', "Python SDK local source path")
    require_contains("ci/check_sorafs_pin_register_jvm_sdk.sh", "RegisterPinManifestInstructionTest", "Kotlin paid-pin builder test")
    require_contains("ci/check_sorafs_pin_register_jvm_sdk.sh", "SorafsRegisterPinManifestBuilderTests", "Java Android paid-pin builder harness")
    require_contains("ci/check_sorafs_pin_register_jvm_sdk.sh", 'JAVA_HOME_OVERRIDE="${SORAFS_PIN_REGISTER_JVM_SDK_JAVA_HOME:-}"', "JVM SDK Java home override variable")
    require_contains("ci/check_sorafs_pin_register_jvm_sdk.sh", "SORAFS_PIN_REGISTER_JVM_SDK_JAVA_HOME must point to a JDK 21 home.", "JVM SDK explicit Java home override rejection")
    require_contains("ci/check_sorafs_pin_register_jvm_sdk.sh", "JAVA_HOME must point to a JDK 21 home for SoraFS pin-register JVM SDK tests.", "JVM SDK inherited Java home rejection")
    require_contains("ci/check_sorafs_pin_register_jvm_sdk.sh", "is_java_21_home()", "JVM SDK JDK 21 resolver")
    require_contains("ci/check_sorafs_pin_register_jvm_sdk.sh", 'version[[:space:]]+\\"21(\\.|\\")', "JVM SDK JDK 21 version matcher")
    require_contains("ci/check_sorafs_pin_register_jvm_sdk.sh", "java -version", "JVM SDK Java version evidence")
    require_contains("ci/check_sorafs_pin_register_swift_sdk.sh", "testRegisterSoraFsPinManifestRejectsMalformedInputsBeforeRequest", "Swift fail-closed test contract")
    require_contains("ci/check_sorafs_pin_register_swift_sdk.sh", 'SWIFTC_BIN="${SORAFS_PIN_REGISTER_SWIFTC_BIN:-swiftc}"', "Swift SDK swiftc override variable")
    require_contains("ci/check_sorafs_pin_register_swift_sdk.sh", '"${SWIFTC_BIN}" --version', "Swift SDK swiftc version evidence")
    require_contains("ci/check_sorafs_pin_register_csharp_sdk.sh", "RegisterSoraFsPinManifestAsync", "C# paid-pin test filter")
    require_contains("ci/check_sorafs_pin_register_csharp_sdk.sh", 'DOTNET_BIN="${SORAFS_PIN_REGISTER_DOTNET_BIN:-dotnet}"', "C# SDK dotnet override variable")
    require_contains("ci/check_sorafs_pin_register_csharp_sdk.sh", 'DOTNET_VERSION="$("${DOTNET_BIN}" --version)"', "C# SDK selected dotnet capture")
    require_contains("ci/check_sorafs_pin_register_csharp_sdk.sh", 'printf \'%s\\n\' "${DOTNET_VERSION}"', "C# SDK dotnet version evidence")
    require_contains("ci/check_sorafs_pin_register_csharp_sdk.sh", "8.0.*) ;;", "C# SDK dotnet 8 matcher")


def check_javascript_contract():
    for path in ("javascript/iroha_js/src/toriiClient.js", "javascript/iroha_js/dist/toriiClient.js"):
        require_contains(path, "async registerSorafsPinManifest(input = {})", "register API")
        require_contains(path, '"/v1/sorafs/pin/register"', "paid-pin endpoint")
        require_contains(path, "buildSorafsPinRegisterPayload", "request normalization")
        require_contains(path, "normalizeSorafsPinRegisterResponse", "typed response normalization")
        require_contains(path, "ambiguous aliases", "ambiguous alias rejection")
        require_contains(path, "contentLength", "content-length validation")
    require_contains("javascript/iroha_js/index.d.ts", "registerSorafsPinManifest(", "TypeScript register declaration")
    require_contains("javascript/iroha_js/index.d.ts", "registerSorafsPinManifestTyped(", "TypeScript typed register declaration")
    require_contains("javascript/iroha_js/test/toriiClient.test.js", "registerSorafsPinManifest rejects ambiguous request field aliases before fetch", "request alias adversarial test")
    require_contains("javascript/iroha_js/test/toriiClient.test.js", "registerSorafsPinManifest rejects ambiguous alias fields before fetch", "alias object adversarial test")
    require_contains("javascript/iroha_js/test/toriiClient.test.js", "registerSorafsPinManifest rejects negative content length before fetch", "content-length adversarial test")
    require_contains("javascript/iroha_js/test/toriiClient.test.js", "registerSorafsPinManifestTyped rejects ambiguous response aliases", "response alias adversarial test")


def check_python_contract():
    require_contains("python/iroha_python/src/iroha_python/__init__.py", "SorafsPinRegisterResponse", "public typed response export")
    for path in ("python/iroha_python/src/iroha_python/client.py",):
        require_contains(path, "def register_sorafs_pin_manifest(", "register API")
        require_contains(path, '"/v1/sorafs/pin/register"', "paid-pin endpoint")
        require_contains(path, "_normalize_sorafs_pin_register_request", "request normalization")
        require_contains(path, "SorafsPinRegisterResponse.from_payload", "typed response normalization")
        require_contains(path, "ambiguous aliases", "ambiguous alias rejection")
    require_contains("python/iroha_python/tests/client_sorafs_pin_register_test.py", "test_register_sorafs_pin_manifest_rejects_duplicate_aliases_before_request", "duplicate request alias adversarial test")
    require_contains("python/iroha_python/tests/client_sorafs_pin_register_test.py", "test_register_sorafs_pin_manifest_rejects_alias_object_with_flat_alias_fields", "alias object adversarial test")
    require_contains("python/iroha_python/tests/client_sorafs_pin_register_test.py", "test_register_sorafs_pin_manifest_rejects_invalid_inputs_before_request", "invalid input adversarial test")
    require_contains("python/iroha_python/tests/client_sorafs_pin_register_test.py", "test_register_sorafs_pin_manifest_typed_rejects_duplicate_response_aliases", "response alias adversarial test")


def check_swift_contract():
    require_contains("IrohaSwift/Sources/IrohaSwift/ToriiClient.swift", "public func registerSoraFsPinManifest(_ requestBody: ToriiSoraFsPinRegisterRequest) async throws -> ToriiSoraFsPinRegisterResponse", "Swift async API")
    require_contains("IrohaSwift/Sources/IrohaSwift/ToriiClient.swift", 'path: "/v1/sorafs/pin/register"', "Swift endpoint")
    require_contains("IrohaSwift/Sources/IrohaSwift/ToriiClient.swift", "requestBody.normalized()", "Swift request normalization")
    require_contains("IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift", "testRegisterSoraFsPinManifestRejectsMalformedInputsBeforeRequest", "Swift malformed input test")
    require_contains("IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift", "XCTAssertFalse(didSendRequest)", "Swift no-request assertion")
    require_contains("IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift", "testRegisterSoraFsPinManifestRejectsMalformedResponse", "Swift malformed response test")


def check_csharp_contract():
    require_contains("csharp/src/Hyperledger.Iroha.Sdk/Torii/ToriiClient.cs", "RegisterSoraFsPinManifestAsync", "C# API")
    require_contains("csharp/src/Hyperledger.Iroha.Sdk/Torii/ToriiClient.cs", '"/v1/sorafs/pin/register"', "C# endpoint")
    require_contains("csharp/src/Hyperledger.Iroha.Sdk/Torii/ToriiClient.cs", "NormalizeSoraFsPinRegisterRequest", "C# request normalization")
    require_contains("csharp/src/Hyperledger.Iroha.Sdk/Torii/ToriiModels.cs", "ToriiSoraFsPinRegisterResponse", "C# response model")
    require_contains("csharp/src/Hyperledger.Iroha.Sdk/Torii/ToriiJsonSerializerContext.cs", "ToriiSoraFsPinRegisterResponse", "C# source-generated JSON context")
    require_contains("csharp/tests/Hyperledger.Iroha.Sdk.Tests/ToriiClientTests.cs", "RegisterSoraFsPinManifestAsyncRejectsMalformedInputsBeforeRequest", "C# malformed input test")
    require_contains("csharp/tests/Hyperledger.Iroha.Sdk.Tests/ToriiClientTests.cs", "Assert.Null(handler.LastRequest)", "C# no-request assertion")
    require_contains("csharp/tests/Hyperledger.Iroha.Sdk.Tests/ToriiClientTests.cs", "RegisterSoraFsPinManifestAsyncRejectsMalformedResponse", "C# malformed response test")


def check_jvm_contract():
    require_contains("kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/core/model/instructions/RegisterPinManifestInstruction.kt", "RegisterPinManifestInstruction", "Kotlin paid-pin instruction")
    require_contains("kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/core/model/instructions/RegisterPinManifestInstructionTest.kt", "builder requires content length", "Kotlin content-length fail-closed test")
    require_contains("kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/core/model/instructions/RegisterPinManifestInstructionTest.kt", "from arguments rejects unsupported storage class", "Kotlin storage-class adversarial test")
    require_contains("kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/core/model/instructions/RegisterPinManifestInstructionTest.kt", "chunker profile rejects nonpositive profile id", "Kotlin chunker profile-id adversarial test")
    require_contains("kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/core/model/instructions/RegisterPinManifestInstructionTest.kt", "from arguments rejects negative chunker multihash code", "Kotlin chunker multihash adversarial test")
    require_contains("java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/instructions/RegisterPinManifestInstruction.java", "RegisterPinManifest", "Java paid-pin instruction")
    require_contains("java/iroha_android/src/test/java/org/hyperledger/iroha/android/sorafs/SorafsRegisterPinManifestBuilderTests.java", "rejectsMissingContentLength", "Java content-length fail-closed test")
    require_contains("java/iroha_android/src/test/java/org/hyperledger/iroha/android/sorafs/SorafsRegisterPinManifestBuilderTests.java", "rejectsFromArgumentsUnsupportedStorageClass();", "Java storage-class adversarial test invocation")
    require_contains("java/iroha_android/src/test/java/org/hyperledger/iroha/android/sorafs/SorafsRegisterPinManifestBuilderTests.java", "rejectsChunkerProfileNonpositiveProfileId();", "Java chunker profile-id adversarial test invocation")
    require_contains("java/iroha_android/src/test/java/org/hyperledger/iroha/android/sorafs/SorafsRegisterPinManifestBuilderTests.java", "rejectsFromArgumentsNegativeChunkerMultihashCode();", "Java chunker multihash adversarial test invocation")


def run_checks():
    for path in required_paths:
        require((root / path).exists(), f"required file is missing: {path}")
    check_workflow()
    check_scripts()
    check_javascript_contract()
    check_python_contract()
    check_swift_contract()
    check_csharp_contract()
    check_jvm_contract()
    print("SoraFS pin-register SDK guard: ok")


def reject_mutation(mutated_path, mutated_text, label):
    text_overrides[mutated_path] = mutated_text
    try:
        run_checks()
    except GuardError as error:
        print(f"negative control rejected {label}")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(f"negative control failed: {label} was not detected")


def workflow_mutation(old, new, label):
    original = read(workflow_path)
    mutated = original.replace(old, new, 1)
    require(mutated != original, f"negative control failed: unable to mutate {label}")
    reject_mutation(workflow_path, mutated, label)


if mode == "--negative-control-workflow-path":
    workflow_mutation('      - "javascript/iroha_js/src/toriiClient.js"\n', "", "workflow path drift")

if mode == "--negative-control-workflow-command":
    workflow_mutation(f"        run: {main_command}", f"        run: {main_command} --skip", "workflow command drift")

if mode == "--negative-control-negative-controls-workflow":
    workflow_mutation(f"          {negative_control_commands[0][1]}\n", "", "negative-control workflow drift")

if mode == "--negative-control-negative-controls-comment-workflow":
    workflow_mutation(
        f"          {negative_control_commands[0][1]}\n",
        f"          # {negative_control_commands[0][1]}\n",
        "commented negative-control workflow drift",
    )

if mode == "--negative-control-negative-controls-order-workflow":
    original = read(workflow_path)
    guard_line = f"        run: {main_command}\n"
    mutated = original.replace(guard_line, "", 1)
    insert = mutated.index("      - name: SoraFS pin-register SDK guard negative controls\n")
    mutated = mutated[:insert] + "      - name: SoraFS pin-register SDK guard\n" + guard_line + mutated[insert:]
    reject_mutation(workflow_path, mutated, "negative-control ordering drift")

if mode == "--negative-control-bytecode-workflow":
    workflow_mutation(
        "      - name: Reject tracked Python bytecode\n"
        f"        run: {bytecode_command}\n",
        "",
        "tracked Python bytecode workflow drift",
    )

if mode == "--negative-control-jvm-sdk-jdk21-script":
    target = "ci/check_sorafs_pin_register_jvm_sdk.sh"
    original = read(target)
    mutated = original.replace("java -version\n", "", 1)
    require(mutated != original, "negative control failed: unable to mutate JVM SDK JDK 21 script evidence")
    reject_mutation(target, mutated, "JVM SDK JDK 21 script drift")

if mode == "--negative-control-jvm-sdk-java-home-override-script":
    target = "ci/check_sorafs_pin_register_jvm_sdk.sh"
    original = read(target)
    mutated = original.replace(
        "SORAFS_PIN_REGISTER_JVM_SDK_JAVA_HOME",
        "SORAFS_PIN_REGISTER_JVM_SDK_JAVA_HOME_DISABLED",
        1,
    )
    require(mutated != original, "negative control failed: unable to mutate JVM SDK Java home override variable")
    reject_mutation(target, mutated, "JVM SDK Java home override drift")

if mode == "--negative-control-jvm-sdk-java-home-reject-script":
    target = "ci/check_sorafs_pin_register_jvm_sdk.sh"
    original = read(target)
    mutated = original.replace(
        "JAVA_HOME must point to a JDK 21 home for SoraFS pin-register JVM SDK tests.",
        "JAVA_HOME is not checked before fallback.",
        1,
    )
    require(mutated != original, "negative control failed: unable to mutate JVM SDK inherited Java home rejection")
    reject_mutation(target, mutated, "JVM SDK inherited Java home rejection drift")

if mode == "--negative-control-jvm-sdk-setup-order-workflow":
    original = read(workflow_path)
    run_line = f"        run: {jvm_command}\n"
    mutated = original.replace(run_line, "", 1)
    require(mutated != original, "negative control failed: unable to mutate JVM SDK setup order")
    insert = mutated.index("      - uses: actions/setup-java@v4\n")
    mutated = (
        mutated[:insert]
        + "      - name: SoraFS pin-register JVM and Java SDK tests\n"
        + run_line
        + mutated[insert:]
    )
    reject_mutation(workflow_path, mutated, "JVM SDK setup ordering drift")

if mode == "--negative-control-csharp-sdk-setup-order-workflow":
    original = read(workflow_path)
    run_line = f"        run: {csharp_command}\n"
    mutated = original.replace(run_line, "", 1)
    require(mutated != original, "negative control failed: unable to mutate C# SDK setup order")
    insert = mutated.index("      - uses: actions/setup-dotnet@v4\n")
    mutated = (
        mutated[:insert]
        + "      - name: SoraFS pin-register C# SDK tests\n"
        + run_line
        + mutated[insert:]
    )
    reject_mutation(workflow_path, mutated, "C# SDK setup ordering drift")

if mode == "--negative-control-csharp-sdk-dotnet-version-script":
    target = "ci/check_sorafs_pin_register_csharp_sdk.sh"
    original = read(target)
    mutated = original.replace('printf \'%s\\n\' "${DOTNET_VERSION}"\n', "", 1)
    require(mutated != original, "negative control failed: unable to mutate C# SDK dotnet version evidence")
    reject_mutation(target, mutated, "C# SDK dotnet version script drift")

if mode == "--negative-control-csharp-sdk-dotnet-override-script":
    target = "ci/check_sorafs_pin_register_csharp_sdk.sh"
    original = read(target)
    mutated = original.replace("SORAFS_PIN_REGISTER_DOTNET_BIN", "SORAFS_PIN_DOTNET_BIN", 1)
    require(mutated != original, "negative control failed: unable to mutate C# SDK dotnet override variable")
    reject_mutation(target, mutated, "C# SDK dotnet override drift")

if mode == "--negative-control-csharp-sdk-dotnet-major-script":
    target = "ci/check_sorafs_pin_register_csharp_sdk.sh"
    original = read(target)
    mutated = original.replace("8.0.*) ;;", "7.0.*) ;;", 1)
    require(mutated != original, "negative control failed: unable to mutate C# SDK dotnet major matcher")
    reject_mutation(target, mutated, "C# SDK dotnet major script drift")

if mode == "--negative-control-js-sdk-node-setup-order-workflow":
    original = read(workflow_path)
    install_block = (
        "      - name: Install JavaScript SDK dependencies\n"
        f"        run: {js_install_command}\n"
    )
    mutated = original.replace(install_block, "", 1)
    require(mutated != original, "negative control failed: unable to move JavaScript SDK install before Node setup")
    insert = mutated.index("      - uses: actions/setup-node@v4\n")
    mutated = mutated[:insert] + install_block + mutated[insert:]
    reject_mutation(workflow_path, mutated, "JavaScript SDK Node setup ordering drift")

if mode == "--negative-control-python-sdk-setup-order-workflow":
    original = read(workflow_path)
    run_line = f"        run: {python_command}\n"
    mutated = original.replace(run_line, "", 1)
    require(mutated != original, "negative control failed: unable to mutate Python SDK setup order")
    insert = mutated.index("      - uses: actions/setup-python@v5\n")
    mutated = (
        mutated[:insert]
        + "      - name: SoraFS pin-register Python SDK tests\n"
        + run_line
        + mutated[insert:]
    )
    reject_mutation(workflow_path, mutated, "Python SDK setup ordering drift")

if mode == "--negative-control-python-sdk-version-script":
    target = "ci/check_sorafs_pin_register_python_sdk.sh"
    original = read(target)
    mutated = original.replace('"${VENV_DIR}/bin/python" --version\n', "", 1)
    require(mutated != original, "negative control failed: unable to mutate Python SDK version evidence")
    reject_mutation(target, mutated, "Python SDK version script drift")

if mode == "--negative-control-python-sdk-override-script":
    target = "ci/check_sorafs_pin_register_python_sdk.sh"
    original = read(target)
    mutated = original.replace("SORAFS_PIN_REGISTER_PYTHON_BIN", "SORAFS_PIN_REGISTER_PY_BIN", 1)
    require(mutated != original, "negative control failed: unable to mutate Python SDK override variable")
    reject_mutation(target, mutated, "Python SDK override drift")

if mode == "--negative-control-python-sdk-resolver-script":
    target = "ci/check_sorafs_pin_register_python_sdk.sh"
    original = read(target)
    mutated = original.replace("resolve_python_311_bin()", "resolve_python_bin()", 1)
    require(mutated != original, "negative control failed: unable to mutate Python SDK resolver")
    reject_mutation(target, mutated, "Python SDK resolver drift")

if mode == "--negative-control-python-sdk-major-script":
    target = "ci/check_sorafs_pin_register_python_sdk.sh"
    original = read(target)
    mutated = original.replace("3.11) ;;", "3.10) ;;")
    require(mutated != original, "negative control failed: unable to mutate Python SDK major matcher")
    reject_mutation(target, mutated, "Python SDK major script drift")

if mode == "--negative-control-python-sdk-venv-rebuild-script":
    target = "ci/check_sorafs_pin_register_python_sdk.sh"
    original = read(target)
    mutated = original.replace('  rm -rf "${VENV_DIR}"\n', "", 1)
    require(mutated != original, "negative control failed: unable to mutate Python SDK stale venv rebuild")
    reject_mutation(target, mutated, "Python SDK stale venv rebuild drift")

if mode == "--negative-control-python-sdk-bytecode-script":
    target = "ci/check_sorafs_pin_register_python_sdk.sh"
    original = read(target)
    mutated = original.replace("export PYTHONDONTWRITEBYTECODE=1\n\n", "", 1)
    require(mutated != original, "negative control failed: unable to mutate Python SDK bytecode guard")
    reject_mutation(target, mutated, "Python SDK bytecode script drift")

if mode == "--negative-control-swift-sdk-version-script":
    target = "ci/check_sorafs_pin_register_swift_sdk.sh"
    original = read(target)
    mutated = original.replace('"${SWIFTC_BIN}" --version\n', "", 1)
    require(mutated != original, "negative control failed: unable to mutate Swift SDK version evidence")
    reject_mutation(target, mutated, "Swift SDK version script drift")

if mode == "--negative-control-swift-sdk-override-script":
    target = "ci/check_sorafs_pin_register_swift_sdk.sh"
    original = read(target)
    mutated = original.replace("SORAFS_PIN_REGISTER_SWIFTC_BIN", "SORAFS_PIN_SWIFTC_BIN", 1)
    require(mutated != original, "negative control failed: unable to mutate Swift SDK compiler override variable")
    reject_mutation(target, mutated, "Swift SDK compiler override drift")

if mode == "--negative-control-js-sdk-node-version-script":
    target = "ci/check_sorafs_pin_register_js_sdk.sh"
    original = read(target)
    mutated = original.replace('printf \'%s\\n\' "${NODE_VERSION}"\n', "", 1)
    require(mutated != original, "negative control failed: unable to mutate JavaScript SDK Node version evidence")
    reject_mutation(target, mutated, "JavaScript SDK Node version script drift")

if mode == "--negative-control-js-sdk-node-override-script":
    target = "ci/check_sorafs_pin_register_js_sdk.sh"
    original = read(target)
    mutated = original.replace("SORAFS_PIN_REGISTER_JS_SDK_NODE_BIN", "SORAFS_PIN_REGISTER_JS_NODE_BIN", 1)
    require(mutated != original, "negative control failed: unable to mutate JavaScript SDK Node override variable")
    reject_mutation(target, mutated, "JavaScript SDK Node override drift")

if mode == "--negative-control-js-sdk-node-resolver-script":
    target = "ci/check_sorafs_pin_register_js_sdk.sh"
    original = read(target)
    mutated = original.replace("resolve_node_20_bin()", "resolve_node_bin()", 1)
    require(mutated != original, "negative control failed: unable to mutate JavaScript SDK Node resolver")
    reject_mutation(target, mutated, "JavaScript SDK Node resolver drift")

if mode == "--negative-control-js-sdk-node-major-script":
    target = "ci/check_sorafs_pin_register_js_sdk.sh"
    original = read(target)
    mutated = original.replace("v20.*) ;;", "v18.*) ;;", 1)
    require(mutated != original, "negative control failed: unable to mutate JavaScript SDK Node major matcher")
    reject_mutation(target, mutated, "JavaScript SDK Node major script drift")

workflow_modes = {
    "--negative-control-swift-sdk-job-workflow": ("  sorafs_pin_register_swift_sdk_check:\n", "  sorafs_pin_register_swift_sdk_check_disabled:\n", "Swift SDK job drift"),
    "--negative-control-swift-sdk-runner-workflow": ("  sorafs_pin_register_swift_sdk_check:\n    runs-on: macos-latest", "  sorafs_pin_register_swift_sdk_check:\n    runs-on: ubuntu-latest", "Swift SDK runner drift"),
    "--negative-control-swift-sdk-script-workflow": (f"        run: {swift_command}", "        run: bash ci/check_sorafs_pin_register_swift_sdk.sh --skip", "Swift SDK script drift"),
    "--negative-control-swift-sdk-needs-workflow": (main_job_needs_line, "    needs: [sorafs_pin_register_jvm_sdk_tests, sorafs_pin_register_csharp_sdk_tests, sorafs_pin_register_javascript_sdk_tests, sorafs_pin_register_python_sdk_tests]", "Swift SDK dependency drift"),
    "--negative-control-jvm-sdk-job-workflow": ("  sorafs_pin_register_jvm_sdk_tests:\n", "  sorafs_pin_register_jvm_sdk_tests_disabled:\n", "JVM SDK job drift"),
    "--negative-control-jvm-sdk-setup-workflow": ("      - uses: actions/setup-java@v4\n", "", "JVM SDK setup drift"),
    "--negative-control-jvm-sdk-distribution-workflow": ('          distribution: "temurin"', '          distribution: "zulu"', "JVM SDK Java distribution drift"),
    "--negative-control-jvm-sdk-java-version-workflow": ('          java-version: "21"', '          java-version: "17"', "JVM SDK Java version drift"),
    "--negative-control-jvm-sdk-script-workflow": (f"        run: {jvm_command}", "        run: bash ci/check_sorafs_pin_register_jvm_sdk.sh --skip", "JVM SDK script drift"),
    "--negative-control-jvm-sdk-needs-workflow": (main_job_needs_line, "    needs: [sorafs_pin_register_swift_sdk_check, sorafs_pin_register_csharp_sdk_tests, sorafs_pin_register_javascript_sdk_tests, sorafs_pin_register_python_sdk_tests]", "JVM SDK dependency drift"),
    "--negative-control-csharp-sdk-job-workflow": ("  sorafs_pin_register_csharp_sdk_tests:\n", "  sorafs_pin_register_csharp_sdk_tests_disabled:\n", "C# SDK job drift"),
    "--negative-control-csharp-sdk-setup-workflow": ("      - uses: actions/setup-dotnet@v4\n", "", "C# SDK setup drift"),
    "--negative-control-csharp-sdk-dotnet-version-workflow": ("          dotnet-version: 8.0.x", "          dotnet-version: 7.0.x", "C# SDK dotnet version drift"),
    "--negative-control-csharp-sdk-script-workflow": (f"        run: {csharp_command}", "        run: bash ci/check_sorafs_pin_register_csharp_sdk.sh --skip", "C# SDK script drift"),
    "--negative-control-csharp-sdk-needs-workflow": (main_job_needs_line, "    needs: [sorafs_pin_register_swift_sdk_check, sorafs_pin_register_jvm_sdk_tests, sorafs_pin_register_javascript_sdk_tests, sorafs_pin_register_python_sdk_tests]", "C# SDK dependency drift"),
    "--negative-control-js-sdk-job-workflow": ("  sorafs_pin_register_javascript_sdk_tests:\n", "  sorafs_pin_register_javascript_sdk_tests_disabled:\n", "JavaScript SDK job drift"),
    "--negative-control-js-sdk-runner-workflow": ("  sorafs_pin_register_javascript_sdk_tests:\n    runs-on: ubuntu-latest", "  sorafs_pin_register_javascript_sdk_tests:\n    runs-on: macos-latest", "JavaScript SDK runner drift"),
    "--negative-control-js-sdk-node-setup-workflow": ("      - uses: actions/setup-node@v4\n", "", "JavaScript SDK Node setup drift"),
    "--negative-control-js-sdk-node-version-workflow": ('          node-version: "20"', '          node-version: "18"', "JavaScript SDK Node version drift"),
    "--negative-control-js-sdk-node-cache-workflow": ("          cache-dependency-path: javascript/iroha_js/package-lock.json", "          cache-dependency-path: javascript/iroha_js/package.json", "JavaScript SDK cache path drift"),
    "--negative-control-js-sdk-install-workflow": (f"        run: {js_install_command}", "        run: npm install --prefix javascript/iroha_js", "JavaScript SDK install drift"),
    "--negative-control-js-sdk-script-workflow": (f"        run: {js_command}", "        run: bash ci/check_sorafs_pin_register_js_sdk.sh --skip", "JavaScript SDK script drift"),
    "--negative-control-js-sdk-needs-workflow": (main_job_needs_line, "    needs: [sorafs_pin_register_swift_sdk_check, sorafs_pin_register_jvm_sdk_tests, sorafs_pin_register_csharp_sdk_tests, sorafs_pin_register_python_sdk_tests]", "JavaScript SDK dependency drift"),
    "--negative-control-python-sdk-job-workflow": ("  sorafs_pin_register_python_sdk_tests:\n", "  sorafs_pin_register_python_sdk_tests_disabled:\n", "Python SDK job drift"),
    "--negative-control-python-sdk-setup-workflow": ("      - uses: actions/setup-python@v5\n", "", "Python SDK setup drift"),
    "--negative-control-python-sdk-version-workflow": ('          python-version: "3.11"', '          python-version: "3.10"', "Python SDK version drift"),
    "--negative-control-python-sdk-script-workflow": (f"        run: {python_command}", "        run: bash ci/check_sorafs_pin_register_python_sdk.sh --skip", "Python SDK script drift"),
    "--negative-control-python-sdk-needs-workflow": (main_job_needs_line, "    needs: [sorafs_pin_register_swift_sdk_check, sorafs_pin_register_jvm_sdk_tests, sorafs_pin_register_csharp_sdk_tests, sorafs_pin_register_javascript_sdk_tests]", "Python SDK dependency drift"),
}

if mode == "--negative-control-js-sdk-install-order-workflow":
    original = read(workflow_path)
    install_line = f"      - name: Install JavaScript SDK dependencies\n        run: {js_install_command}\n"
    mutated = original.replace(install_line, "", 1)
    mutated = mutated.replace(
        f"        run: {js_command}\n",
        f"        run: {js_command}\n" + install_line,
        1,
    )
    reject_mutation(workflow_path, mutated, "JavaScript SDK install ordering drift")

if mode == "--negative-control-js-sdk-test-order-workflow":
    original = read(workflow_path)
    test_line = f"      - name: SoraFS pin-register JavaScript SDK tests\n        run: {js_command}\n"
    mutated = original.replace(test_line, "", 1)
    mutated = mutated.replace(
        f"        run: {main_command}\n",
        f"        run: {main_command}\n" + test_line,
        1,
    )
    reject_mutation(workflow_path, mutated, "JavaScript SDK test ordering drift")

if mode in workflow_modes:
    old, new, label = workflow_modes[mode]
    workflow_mutation(old, new, label)

source_modes = {
    "--negative-control-js-source-endpoint": (
        "javascript/iroha_js/src/toriiClient.js",
        '"/v1/sorafs/pin/register"',
        '"/v1/sorafs/pin/register-disabled"',
        "JavaScript source endpoint drift",
    ),
    "--negative-control-js-adversarial-test": (
        "javascript/iroha_js/test/toriiClient.test.js",
        "registerSorafsPinManifest rejects ambiguous request field aliases before fetch",
        "registerSorafsPinManifest request alias test disabled",
        "JavaScript adversarial test drift",
    ),
    "--negative-control-python-adversarial-test": (
        "python/iroha_python/tests/client_sorafs_pin_register_test.py",
        "test_register_sorafs_pin_manifest_rejects_duplicate_aliases_before_request",
        "test_register_sorafs_pin_manifest_duplicate_aliases_disabled",
        "Python adversarial test drift",
    ),
    "--negative-control-swift-contract-test": (
        "IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift",
        "testRegisterSoraFsPinManifestRejectsMalformedInputsBeforeRequest",
        "testRegisterSoraFsPinManifestRejectsMalformedInputsDisabled",
        "Swift contract test drift",
    ),
    "--negative-control-csharp-malformed-response-test": (
        "csharp/tests/Hyperledger.Iroha.Sdk.Tests/ToriiClientTests.cs",
        "RegisterSoraFsPinManifestAsyncRejectsMalformedResponse",
        "RegisterSoraFsPinManifestAsyncMalformedResponseDisabled",
        "C# malformed response test drift",
    ),
    "--negative-control-kotlin-builder-test": (
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/core/model/instructions/RegisterPinManifestInstructionTest.kt",
        "builder requires content length",
        "builder content length test disabled",
        "Kotlin builder test drift",
    ),
    "--negative-control-kotlin-chunker-unsigned-test": (
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/core/model/instructions/RegisterPinManifestInstructionTest.kt",
        "chunker profile rejects nonpositive profile id",
        "chunker profile unsigned test disabled",
        "Kotlin chunker unsigned test drift",
    ),
    "--negative-control-java-builder-test": (
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/sorafs/SorafsRegisterPinManifestBuilderTests.java",
        "rejectsFromArgumentsUnsupportedStorageClass();",
        "rejectsFromArgumentsUnsupportedStorageClassDisabled();",
        "Java builder test drift",
    ),
    "--negative-control-java-chunker-unsigned-test": (
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/sorafs/SorafsRegisterPinManifestBuilderTests.java",
        "rejectsChunkerProfileNonpositiveProfileId();",
        "rejectsChunkerProfileNonpositiveProfileIdDisabled();",
        "Java chunker unsigned test drift",
    ),
}

if mode in source_modes:
    path, old, new, label = source_modes[mode]
    original = read(path)
    mutated = original.replace(old, new, 1)
    require(mutated != original, f"negative control failed: unable to mutate {label}")
    reject_mutation(path, mutated, label)

if mode:
    raise SystemExit(f"unknown mode: {mode}")

try:
    run_checks()
except GuardError as error:
    raise SystemExit(f"error: {error}") from error
PY
