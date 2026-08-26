from __future__ import annotations

import re
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[2]
FROZEN_LOCK_SHA256 = (
    "cd9e829e454171f17540abeb7fd1aa14129252082bd8b076a0199b0ffa4e3f79"
)
TRACKED_ROOT_LOCK_SHA256 = (
    "c90b3659d6cb44cd1d6f9e75e7b98aacc0d30bbe23041d4e6e109e8a206fa76b"
)


def read(relative: str) -> str:
    return (ROOT / relative).read_text(encoding="utf-8")


def jvm_job(workflow: str) -> str:
    match = re.search(
        r"(?ms)^  privacy_jvm_sdk_tests:\n(.*?)(?=^  privacy_csharp_sdk_tests:)",
        workflow,
    )
    assert match is not None
    return match.group(1)


def csharp_job(workflow: str) -> str:
    match = re.search(
        r"(?ms)^  privacy_csharp_sdk_tests:\n(.*?)(?=^  privacy_javascript_sdk_tests:)",
        workflow,
    )
    assert match is not None
    return match.group(1)


def javascript_job(workflow: str) -> str:
    match = re.search(
        r"(?ms)^  privacy_javascript_sdk_tests:\n(.*?)(?=^  privacy_python_sdk_tests:)",
        workflow,
    )
    assert match is not None
    return match.group(1)


def swift_job(workflow: str) -> str:
    match = re.search(
        r"(?ms)^  privacy_swift_sdk_parse:\n(.*?)(?=^  privacy_jvm_sdk_tests:)",
        workflow,
    )
    assert match is not None
    return match.group(1)


def require_fail_closed_tests(kotlin: str, java: str) -> None:
    assert "IROHA_REQUIRE_PRIVACY_EXACT12_NATIVE" not in kotlin
    assert "IROHA_REQUIRE_PRIVACY_EXACT12_NATIVE" not in java
    assert re.search(r"if\s*\(\s*!available\s*\)\s*return\b", kotlin) is None
    assert re.search(
        r"if\s*\(\s*!available\s*\)\s*\{\s*return;\s*\}", java
    ) is None
    assert kotlin.count(
        "ABI-23 connect_norito_bridge with compiled-profile catalog JNI exports is required"
    ) == 1
    assert kotlin.count(
        "ABI-23 connect_norito_bridge with exact-12 fixture JNI exports is required"
    ) == 1
    assert java.count(
        "ABI-23 connect_norito_bridge with compiled-profile catalog JNI exports is required"
    ) == 1
    assert java.count(
        "ABI-23 connect_norito_bridge with exact-12 fixture JNI exports is required"
    ) == 1
    assert kotlin.count("assertTrue(\n            available,") == 2
    assert java.count("if (!available) {") == 2
    assert java.count("throw new AssertionError(") >= 2


def test_privacy_jvm_gate_builds_and_authenticates_native_abi23() -> None:
    gate = read("ci/check_privacy_jvm_sdk.sh")
    assert f'FROZEN_CARGO_LOCK_SHA256="{FROZEN_LOCK_SHA256}"' in gate
    assert f'TRACKED_ROOT_CARGO_LOCK_SHA256="{TRACKED_ROOT_LOCK_SHA256}"' in gate
    assert '[[ "${RUSTC_VERSION}" == rustc\\ 1.93.1\\ * ]]' in gate
    assert '"${CARGO_BIN}" build --locked -p connect_norito_bridge --lib' in gate
    assert 'export NORITO_SKIP_BINDINGS_SYNC=1' in gate
    assert gate.count('"${ABI22_CHECKER}" verify') == 6
    assert gate.count('"${ABI22_CHECKER}" record') == 2
    assert '--sdk c-jni' in gate
    assert '--source-root "${ROOT_DIR}"' in gate
    assert 'export IROHA_NATIVE_LIBRARY_PATH="${NATIVE_LIBRARY_DIR}"' in gate
    assert 'export IROHA_REQUIRE_PRIVACY_EXACT12_NATIVE=1' in gate
    assert '--sdk csharp' in gate
    assert 'PRIVACY_JVM_NATIVE_EXPORT_DIR' in gate

    build = gate.index('"${CARGO_BIN}" build --locked')
    record = gate.index('"${ABI22_CHECKER}" record')
    tests = gate.index('./gradlew --no-daemon -q :core-jvm:jar :core-jvm:test')
    final_verify = gate.rindex('"${ABI22_CHECKER}" verify')
    assert build < record < tests < final_verify
    assert 'install -m 600 "${SELECTED_CARGO_LOCK}" "${ROOT_DIR}/Cargo.lock"' not in gate
    assert 'install -m 400 "${SELECTED_CARGO_LOCK}"' in gate


def test_privacy_jvm_workflow_provisions_exact_native_build_lane() -> None:
    workflow = read(".github/workflows/pr_privacy_sdk_guard.yml")
    job = jvm_job(workflow)
    assert "timeout-minutes: 60" in job
    assert "python-version: \"3.12\"" in job
    assert '"1.93.1-x86_64-unknown-linux-gnu"' in job
    assert "ci/privacy_sdk_cargo_lockfile.sh provision-ci" in job
    assert job.count("ci/privacy_sdk_cargo_lockfile.sh verify-ci") == 2
    assert "cargo fetch --locked" in job
    assert "run: ci/check_privacy_jvm_sdk.sh" in job
    assert "PRIVACY_JVM_SDK_PYTHON_BIN:" in job
    assert "actions/upload-artifact@" in job
    assert "PRIVACY_JVM_NATIVE_EXPORT_DIR:" in job
    for dependency in (
        "scripts/check_native_sdk_abi22_artifact.py",
        "scripts/compute_workspace_source_manifest.py",
        "scripts/tests/check_privacy_jvm_native_gate_test.py",
    ):
        assert f'- "{dependency}"' in workflow


def test_csharp_lane_consumes_the_same_authenticated_native_bytes() -> None:
    workflow = read(".github/workflows/pr_privacy_sdk_guard.yml")
    job = csharp_job(workflow)
    assert "needs: privacy_jvm_sdk_tests" in job
    assert 'IROHA_REQUIRE_PRIVACY_EXACT12_NATIVE: "1"' in job
    assert "actions/download-artifact@" in job
    assert "privacy-jvm-native-abi22-${{ github.sha }}" in job
    assert "native-sdk-abi22-csharp.json" in job
    assert job.count("check_native_sdk_abi22_artifact.py verify") == 2
    assert FROZEN_LOCK_SHA256 in job
    assert TRACKED_ROOT_LOCK_SHA256 in job
    assert 'install -m 600 "$input/Cargo.lock" Cargo.lock' not in job
    assert "run: ci/check_privacy_csharp_sdk.sh" in job

    tests = read(
        "csharp/tests/Hyperledger.Iroha.Sdk.Tests/PrivacyNativeTests.cs"
    )
    assert "IROHA_REQUIRE_PRIVACY_EXACT12_NATIVE" not in tests
    assert "GetEnvironmentVariable" not in tests
    assert tests.count(
        "Assert.True(\n            PrivacyNative.IsAvailable(),"
    ) == 2
    assert "WhenAvailable" not in tests


def test_javascript_lane_builds_and_executes_real_napi_abi23() -> None:
    workflow = read(".github/workflows/pr_privacy_sdk_guard.yml")
    job = javascript_job(workflow)
    assert "needs: privacy_jvm_sdk_tests" in job
    assert "timeout-minutes: 60" in job
    assert 'node-version: "20"' in job
    assert 'python-version: "3.12"' in job
    assert '"1.93.1-x86_64-unknown-linux-gnu"' in job
    assert "actions/download-artifact@" in job
    assert FROZEN_LOCK_SHA256 in job
    assert TRACKED_ROOT_LOCK_SHA256 in job
    assert "not yet requalified" in job
    assert "install -m 600" not in job
    assert "cargo fetch --locked" in job
    assert "run: ci/check_privacy_js_sdk.sh" in job

    gate = read("ci/check_privacy_js_sdk.sh")
    assert f'FROZEN_CARGO_LOCK_SHA256="{FROZEN_LOCK_SHA256}"' in gate
    assert 'scripts/build-native.mjs' in gate
    assert 'scripts/copy-native.mjs' in gate
    assert gate.count('"${ABI22_CHECKER}" verify') == 2
    assert '"${ABI22_CHECKER}" record' in gate
    assert '--sdk node' in gate
    assert 'test/privacyNative.integration.test.js' in gate
    assert 'export IROHA_JS_NATIVE_DIR=' in gate
    assert 'export CARGO_NET_OFFLINE=true' in gate

    integration = read(
        "javascript/iroha_js/test/privacyNative.integration.test.js"
    )
    assert "getNativeBinding()" in integration
    assert "isPrivacyNativeAvailable(), true" in integration
    assert "globalThis.__IROHA_NATIVE_BINDING__, undefined" in integration
    assert "withNativeBinding" not in integration
    assert "privacyValidateCompiledProfileCatalogV1" in integration


def test_python_lane_authenticates_and_executes_real_pyo3_abi23() -> None:
    gate = read("ci/check_privacy_python_sdk.sh")
    assert f'FROZEN_CARGO_LOCK_SHA256="{FROZEN_LOCK_SHA256}"' in gate
    assert '"${ABI22_CHECKER}" record' in gate
    assert gate.count('"${ABI22_CHECKER}" verify') == 2
    assert '--sdk python' in gate
    assert '--python "${VENV_DIR}/bin/python"' in gate
    assert 'materialize_workspace_lock_for_native_evidence' not in gate
    assert 'remove_workspace_lock_after_native_evidence' not in gate
    assert 'tests/privacy_native_integration_test.py' in gate

    integration = read(
        "python/iroha_python/tests/privacy_native_integration_test.py"
    )
    assert 'import_module("iroha_python._crypto")' in integration
    assert "connect_norito_bridge_abi_version()" in integration
    assert "is_privacy_native_available()" in integration
    assert "privacy_validate_compiled_profile_catalog_v1" in integration
    assert "monkeypatch" not in integration

    workflow = read(".github/workflows/pr_privacy_sdk_guard.yml")
    assert (
        '- "python/iroha_python/tests/privacy_native_integration_test.py"'
        in workflow
    )


def test_swift_lane_rebuilds_external_xcframework_and_requires_native_abi23() -> None:
    workflow = read(".github/workflows/pr_privacy_sdk_guard.yml")
    job = swift_job(workflow)
    assert "needs: privacy_jvm_sdk_tests" in job
    assert "runs-on: macos-14" in job
    assert "timeout-minutes: 120" in job
    assert "dtolnay/rust-toolchain" not in job
    assert '"1.93.1-aarch64-apple-darwin"' in job
    assert "RUSTUP_TOOLCHAIN=1.93.1-aarch64-apple-darwin" in job
    assert "python3 -I -S" not in job
    assert "actions/download-artifact@" in job
    assert FROZEN_LOCK_SHA256 in job
    assert TRACKED_ROOT_LOCK_SHA256 in job
    assert "not yet requalified" in job
    assert "install -m 600" not in job
    assert "MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT=1" in job
    assert "NORITO_BRIDGE_OUT_DIR=" in job
    assert "NORITO_BRIDGE_BUILD_DIR=" in job
    assert "cargo fetch --locked" in job
    assert "chmod -R a-w" in job
    assert "scripts/build_norito_xcframework.sh" in job
    assert "run: ci/check_privacy_swift_sdk.sh" in job
    assert "Revalidate frozen Swift inputs and ABI23 artifacts" in job
    assert job.count("scripts/check_mobile_sdk_artifacts.sh --apple-only") == 1

    gate = read("ci/check_privacy_swift_sdk.sh")
    assert f'FROZEN_CARGO_LOCK_SHA256="{FROZEN_LOCK_SHA256}"' in gate
    assert 'MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT:-}" != "1"' in gate
    assert "must remain outside the source tree" in gate
    assert "xcode-select -p" in gate
    assert "xcodebuild -version" in gate
    assert 'bash "${APPLE_ARTIFACT_CHECKER}" --apple-only' in gate
    assert "--disable-automatic-resolution" in gate
    assert '--scratch-path "${SWIFT_SCRATCH_DIRECTORY}"' in gate

    tests = read(
        "IrohaSwift/Tests/IrohaSwiftTests/PrivacyNativeBridgeTests.swift"
    )
    assert "guard PrivacyNativeBridge.isNativeAvailable else" not in tests
    assert tests.count(
        "XCTAssertTrue(\n            PrivacyNativeBridge.isNativeAvailable,"
    ) == 2


def test_kotlin_and_java_privacy_native_tests_cannot_skip_jni() -> None:
    require_fail_closed_tests(
        read(
            "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/privacy/"
            "PrivacyNativeBridgeTest.kt"
        ),
        read(
            "java/iroha_android/src/test/java/org/hyperledger/iroha/android/privacy/"
            "PrivacyNativeBridgeTest.java"
        ),
    )


@pytest.mark.parametrize(
    ("language", "mutation"),
    (
        (
            "kotlin",
            '\n        if (System.getenv("IROHA_REQUIRE_PRIVACY_EXACT12_NATIVE") == "1") return\n',
        ),
        ("kotlin", "\n        if (!available) return\n"),
        ("java", "\n    if (!available) { return; }\n"),
        (
            "java",
            '\n    if ("1".equals(System.getenv("IROHA_REQUIRE_PRIVACY_EXACT12_NATIVE"))) return;\n',
        ),
    ),
)
def test_skip_regressions_are_hostile_negative_controls(
    language: str, mutation: str
) -> None:
    kotlin = read(
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/privacy/"
        "PrivacyNativeBridgeTest.kt"
    )
    java = read(
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/privacy/"
        "PrivacyNativeBridgeTest.java"
    )
    if language == "kotlin":
        kotlin += mutation
    else:
        java += mutation
    with pytest.raises(AssertionError):
        require_fail_closed_tests(kotlin, java)
