from __future__ import annotations

import re
import sys
import types
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[2]



def read(relative: str) -> str:
    return (ROOT / relative).read_text(encoding="utf-8")


@pytest.fixture(scope="module")
def native_selection_guard():
    owner = ROOT / "ci/check_privacy_sdk_guard.sh"
    body = owner.read_text().split("<<'PY'\n", 1)[1].split("\nPY\n", 1)[0]
    module = types.ModuleType("privacy_jvm_native_selection_contract")
    sys.modules[module.__name__] = module
    original_argv = sys.argv
    try:
        sys.argv = [str(owner), str(ROOT), ""]
        exec(compile(body[:body.index("\nif mode:")], str(owner), "exec"), module.__dict__)
    finally:
        sys.argv = original_argv
        del sys.modules[module.__name__]
    return module


def test_native_selection_preserves_old_suites_and_adds_canonical_consumers(native_selection_guard) -> None:
    expected = (
        "org.hyperledger.iroha.sdk.privacy.PrivacyNativeBridgeTest",
        "org.hyperledger.iroha.sdk.privacy.PrivacyExact12FixtureCodecV1Test",
        "org.hyperledger.iroha.sdk.privacy.PrivacyExact12FixtureJavaConsumerTest",
        "org.hyperledger.iroha.sdk.core.model.zk.VerifyingKeyBackendTagTest",
        "org.hyperledger.iroha.sdk.core.model.zk.VerifyingKeyRecordDescriptionTest",
        "org.hyperledger.iroha.sdk.core.model.instructions.VerifyingKeyInstructionBuildersTest",
        "org.hyperledger.iroha.sdk.core.model.instructions.ProofAttachmentTest",
        "org.hyperledger.iroha.sdk.address.AccountAddressNativeTest",
        "org.hyperledger.iroha.sdk.address.AccountAddressNativeUnavailableTest",
        "org.hyperledger.iroha.sdk.address.AccountAddressTest",
        "org.hyperledger.iroha.sdk.address.AccountIdLiteralTest",
        "org.hyperledger.iroha.sdk.core.model.instructions.CanonicalMultisigWireParityTest",
        "org.hyperledger.iroha.sdk.core.model.instructions.KaigiWirePayloadV1Test",
        "org.hyperledger.iroha.sdk.core.model.instructions.KaigiInstructionValidationTest",
        "org.hyperledger.iroha.sdk.privacy.PrivacyNativeBridgeJavaConsumerTest",
        "org.hyperledger.iroha.sdk.privacy.ConfidentialNoteJavaConsumerTest",
        "org.hyperledger.iroha.sdk.privacy.ZkAssetMerklePathJavaConsumerTest",
        "org.hyperledger.iroha.sdk.privacy.PrivacyRetiredWitnessBoundaryJavaConsumerTest",
    )
    assert native_selection_guard.JVM_NATIVE_TEST_SELECTIONS == expected
    errors = []
    native_selection_guard._check_jvm_native_test_selection(read("ci/check_privacy_jvm_sdk.sh"), errors)
    assert errors == []


@pytest.mark.parametrize("index", range(18))
def test_native_selection_rejects_each_omitted_old_or_new_suite(native_selection_guard, index: int) -> None:
    name = native_selection_guard.JVM_NATIVE_TEST_SELECTIONS[index]
    gate = read("ci/check_privacy_jvm_sdk.sh")
    changed, count = re.subn(r"(?m)^  --tests " + re.escape(name) + r"(?: \\)?\n", "", gate)
    assert count == 1
    with pytest.raises(native_selection_guard.GuardFailure, match="privacy JVM native gate must execute every reviewed Exact12, canonical account and Kaigi test selection"):
        native_selection_guard.check({"ci/check_privacy_jvm_sdk.sh": changed})


@pytest.mark.parametrize("mutation", ("comment", "duplicate", "outside-command", "unexecuted-command"))
def test_native_selection_rejects_nonexecuted_or_duplicate_selection(native_selection_guard, mutation: str) -> None:
    gate = read("ci/check_privacy_jvm_sdk.sh")
    line = "  --tests org.hyperledger.iroha.sdk.address.AccountAddressNativeTest \\\n"
    assert gate.count(line) == 1
    if mutation == "comment":
        changed = gate.replace(line, "#" + line, 1)
    elif mutation == "duplicate":
        changed = gate.replace(line, line + line, 1)
    elif mutation == "outside-command":
        changed = gate.replace(line, "", 1) + "\n" + line
    else:
        changed = gate.replace("./gradlew --no-daemon -q :core-jvm:jar :core-jvm:test", "# ./gradlew --no-daemon -q :core-jvm:jar :core-jvm:test", 1)
    errors = []
    native_selection_guard._check_jvm_native_test_selection(changed, errors)
    assert errors


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
    assert 'source "${ROOT_DIR}/ci/privacy_sdk_cargo_lockfile.sh"' in gate
    assert "${PRIVACY_SDK_CANONICAL_CARGO_LOCK_SHA256}" in gate
    assert "FROZEN_CARGO_LOCK_SHA256=" not in gate
    assert "TRACKED_ROOT_CARGO_LOCK_SHA256=" not in gate
    assert '[[ "${RUSTC_VERSION}" == rustc\\ 1.93.1\\ * ]]' in gate
    assert '"${CARGO_BIN}" build --locked -p connect_norito_bridge --lib' in gate
    assert 'export NORITO_SKIP_BINDINGS_SYNC=1' in gate
    assert gate.count('"${ABI23_CHECKER}" verify') == 6
    assert gate.count('"${ABI23_CHECKER}" record') == 2
    assert '--sdk c-jni' in gate
    assert '--source-root "${ROOT_DIR}"' in gate
    assert 'export IROHA_NATIVE_LIBRARY_PATH="${NATIVE_LIBRARY_DIR}"' in gate
    assert 'export IROHA_REQUIRE_PRIVACY_EXACT12_NATIVE=1' in gate
    assert '--sdk csharp' in gate
    assert 'PRIVACY_JVM_NATIVE_EXPORT_DIR' in gate

    build = gate.index('"${CARGO_BIN}" build --locked')
    record = gate.index('"${ABI23_CHECKER}" record')
    tests = gate.index('./gradlew --no-daemon -q :core-jvm:jar :core-jvm:test')
    final_verify = gate.rindex('"${ABI23_CHECKER}" verify')
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
        "scripts/check_native_sdk_abi23_artifact.py",
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
    assert "privacy-jvm-native-abi23-${{ github.sha }}" in job
    assert "native-sdk-abi23-csharp.json" in job
    assert job.count("check_native_sdk_abi23_artifact.py verify") == 2
    assert job.count("${PRIVACY_SDK_CANONICAL_CARGO_LOCK_SHA256}") >= 2
    assert "source ci/privacy_sdk_cargo_lockfile.sh" in job
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
    assert job.count("${PRIVACY_SDK_CANONICAL_CARGO_LOCK_SHA256}") >= 2
    assert "source ci/privacy_sdk_cargo_lockfile.sh" in job
    assert "not yet requalified" not in job
    assert "install -m 600" not in job
    assert (
        'RUSTC_BOOTSTRAP=1 cargo -Z unstable-options fetch --locked --lockfile-path '
        '"$IROHA_PRIVACY_RELEASE_CARGO_LOCKFILE_PATH"'
    ) in job
    assert "run: ci/check_privacy_js_sdk.sh" in job

    gate = read("ci/check_privacy_js_sdk.sh")
    assert "${PRIVACY_SDK_CANONICAL_CARGO_LOCK_SHA256}" in gate
    assert 'source "${SCRIPT_DIR}/privacy_sdk_cargo_lockfile.sh"' in gate
    assert 'scripts/build-native.mjs' in gate
    assert 'scripts/copy-native.mjs' in gate
    assert gate.count('"${ABI23_CHECKER}" verify') == 2
    assert '"${ABI23_CHECKER}" record' in gate
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
    assert "${PRIVACY_SDK_CANONICAL_CARGO_LOCK_SHA256}" in gate
    assert 'source "${SCRIPT_DIR}/privacy_sdk_cargo_lockfile.sh"' in gate
    assert '"${ABI23_CHECKER}" record' in gate
    assert gate.count('"${ABI23_CHECKER}" verify') == 2
    assert '--sdk python' in gate
    assert '--python "${VENV_DIR}/bin/python"' in gate
    assert 'materialize_workspace_lock_for_native_evidence' not in gate
    assert 'remove_workspace_lock_after_native_evidence' not in gate
    assert 'tests/privacy_native_integration_test.py' in gate

    integration = read(
        "python/iroha_python/tests/privacy_native_integration_test.py"
    )
    assert "from iroha_native import load_crypto_extension" in integration
    assert "native = load_crypto_extension()" in integration
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
    assert "timeout-minutes: 360" in job
    assert "dtolnay/rust-toolchain" not in job
    assert '"1.93.1-aarch64-apple-darwin"' in job
    assert "RUSTUP_TOOLCHAIN=1.93.1-aarch64-apple-darwin" in job
    assert "python3 -I -S" not in job
    assert "actions/download-artifact@" in job
    assert job.count("${PRIVACY_SDK_CANONICAL_CARGO_LOCK_SHA256}") >= 2
    assert "source ci/privacy_sdk_cargo_lockfile.sh" in job
    assert "not yet requalified" not in job
    assert "install -m 600" not in job
    assert "MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT=1" in job
    assert "NORITO_BRIDGE_OUT_DIR=" in job
    assert "NORITO_BRIDGE_BUILD_DIR=" in job
    assert (
        'RUSTC_BOOTSTRAP=1 cargo -Z unstable-options fetch --locked --lockfile-path '
        '"$IROHA_PRIVACY_RELEASE_CARGO_LOCKFILE_PATH"'
    ) in job
    assert "chmod -R a-w" in job
    assert "scripts/build_norito_xcframework.sh" in job
    assert "run: ci/check_privacy_swift_sdk.sh" in job
    assert "Revalidate frozen Swift inputs and ABI23 artifacts" in job
    assert job.count("scripts/check_mobile_sdk_artifacts.sh --apple-only") == 1

    gate = read("ci/check_privacy_swift_sdk.sh")
    assert "${PRIVACY_SDK_CANONICAL_CARGO_LOCK_SHA256}" in gate
    assert 'source "${ROOT_DIR}/ci/privacy_sdk_cargo_lockfile.sh"' in gate
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
            "kotlin/core-jvm/src/test/java/org/hyperledger/iroha/sdk/privacy/"
            "PrivacyNativeBridgeJavaConsumerTest.java"
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
        "kotlin/core-jvm/src/test/java/org/hyperledger/iroha/sdk/privacy/"
        "PrivacyNativeBridgeJavaConsumerTest.java"
    )
    if language == "kotlin":
        kotlin += mutation
    else:
        java += mutation
    with pytest.raises(AssertionError):
        require_fail_closed_tests(kotlin, java)


@pytest.mark.parametrize("mutation", ("path-prepend", "ambient-java", "ambient-javac", "retired-owner", "missing-class-contract", "reflection", "internal-alias"))
def test_jvm_java_owner_and_jdk_contract_rejects_observed_regressions(native_selection_guard, mutation: str) -> None:
    gate = read("ci/check_privacy_jvm_sdk.sh")
    consumer = read("kotlin/core-jvm/src/test/java/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridgeJavaConsumerTest.java")
    errors = []
    native_selection_guard._check_jvm_java_owner_and_toolchain(gate, consumer, errors)
    assert errors == []
    if mutation == "path-prepend":
        gate = gate.replace('export JAVA_HOME', 'export JAVA_HOME\nexport PATH="${JAVA_HOME}/bin:${PATH}"', 1)
    elif mutation == "ambient-java":
        gate = gate.replace('"${JAVA_HOME}/bin/java"', 'java')
    elif mutation == "ambient-javac":
        gate = gate.replace('"${JAVA_HOME}/bin/javac"', 'javac')
    elif mutation == "missing-class-contract":
        gate = gate.replace('scripts/check_privacy_jvm_class_contract.py', 'scripts/absent_class_contract.py')
    elif mutation == "reflection":
        consumer += "\n// java.lang.reflect.Method\n"
    elif mutation == "internal-alias":
        consumer += "\n// requireCompiledProfileCatalog(candidate)\n"
    else:
        consumer = consumer.replace('package org.hyperledger.iroha.sdk.privacy;', 'package org.hyperledger.iroha.android.privacy;')
    native_selection_guard._check_jvm_java_owner_and_toolchain(gate, consumer, errors)
    assert errors


def test_selected_jdk_invocation_preserves_original_authenticated_path(tmp_path) -> None:
    import os
    import subprocess
    jdk = tmp_path / "jdk"
    (jdk / "bin").mkdir(parents=True)
    (jdk / "bin/java").write_text('#!/bin/sh\nprintf "%s\\n" "$PATH"\n')
    (jdk / "bin/java").chmod(0o700)
    gate = read("ci/check_privacy_jvm_sdk.sh")
    start = gate.index('JAVA_HOME="$(resolve_java_home)"')
    end = gate.index('\ncd "${ROOT_DIR}/kotlin"', start)
    body = gate[start:end]
    original_path = "/authenticated-wrapper:/authenticated-toolchain:/usr/bin:/bin"
    env = dict(os.environ, PATH=original_path, TEST_JDK=str(jdk))
    result = subprocess.run(['/bin/bash', '-euc', 'resolve_java_home() { printf "%s\\n" "$TEST_JDK"; }\n' + body + '\n[[ "$PATH" == "$EXPECTED_PATH" ]]',], env=dict(env, EXPECTED_PATH=original_path), capture_output=True, text=True, timeout=10)
    assert result.returncode == 0, result.stderr
    assert result.stdout.strip() == original_path
    failed = subprocess.run(['/bin/bash', '-euc', 'resolve_java_home() { printf "%s\\n" "$TEST_JDK"; }\n' + body.replace('export JAVA_HOME', 'export JAVA_HOME\nexport PATH="${JAVA_HOME}/bin:${PATH}"') + '\n[[ "$PATH" == "$EXPECTED_PATH" ]]'], env=dict(env, EXPECTED_PATH=original_path), capture_output=True, text=True, timeout=10)
    assert failed.returncode != 0


@pytest.mark.parametrize("owner_index", range(18))
def test_confidential_retirement_rejects_every_reintroduced_java_owner(native_selection_guard, tmp_path, owner_index):
    consumers = {
        name: read("kotlin/core-jvm/src/test/java/org/hyperledger/iroha/sdk/privacy/" + name + "JavaConsumerTest.java")
        for name in ("ConfidentialNote", "ZkAssetMerklePath", "PrivacyRetiredWitnessBoundary")
    }
    errors = []
    native_selection_guard._check_jvm_confidential_owner_closure(tmp_path, consumers, errors)
    assert errors == []
    owner = native_selection_guard.RETIRED_JAVA_PRIVACY_OWNERS[owner_index]
    path = tmp_path / "java/iroha_android/src/main/java/org/hyperledger/iroha/android/privacy" / (owner + ".java")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("retired duplicate", encoding="utf-8")
    native_selection_guard._check_jvm_confidential_owner_closure(tmp_path, consumers, errors)
    assert errors == ["retired duplicate Java privacy owner must remain absent: " + owner]


@pytest.mark.parametrize("name", ("ConfidentialNote", "ZkAssetMerklePath", "PrivacyRetiredWitnessBoundary"))
@pytest.mark.parametrize("mutation", ("missing-group", "old-owner", "reflection"))
def test_confidential_java_migration_rejects_missing_groups_and_duplicate_owners(native_selection_guard, tmp_path, name, mutation):
    consumers = {
        item: read("kotlin/core-jvm/src/test/java/org/hyperledger/iroha/sdk/privacy/" + item + "JavaConsumerTest.java")
        for item in ("ConfidentialNote", "ZkAssetMerklePath", "PrivacyRetiredWitnessBoundary")
    }
    if mutation == "missing-group":
        consumers[name] = consumers[name].replace("@Test", "", 1)
    elif mutation == "old-owner":
        consumers[name] = consumers[name].replace("org.hyperledger.iroha.sdk.privacy", "org.hyperledger.iroha.android.privacy", 1)
    else:
        consumers[name] += "\n// java.lang.reflect.Method"
    errors = []
    native_selection_guard._check_jvm_confidential_owner_closure(tmp_path, consumers, errors)
    assert errors == ["original " + name + " Java assertions must use canonical Kotlin capabilities"]
