from __future__ import annotations

import os
import re
import shutil
import subprocess
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[2]
FROZEN_LOCK_SHA256 = (
    "ccf4acebfe63ad981193b87afd559c195d8a67642d9536b8082f77bbf24a11f0"
)
TRACKED_ROOT_LOCK_SHA256 = (
    "ad0d209abaa51d4c77a9e67ccbb0c7660a0f8b7b5dbe3e3fbe4a70e142711bf7"
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


def swift_slice_job(workflow: str) -> str:
    match = re.search(
        r"(?ms)^  privacy_swift_sdk_slice:\n(.*?)(?=^  privacy_swift_sdk_parse:)",
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
        "ABI-22 connect_norito_bridge with compiled-profile catalog JNI exports is required"
    ) == 1
    assert kotlin.count(
        "ABI-22 connect_norito_bridge with exact-12 fixture JNI exports is required"
    ) == 1
    assert java.count(
        "ABI-22 connect_norito_bridge with compiled-profile catalog JNI exports is required"
    ) == 1
    assert java.count(
        "ABI-22 connect_norito_bridge with exact-12 fixture JNI exports is required"
    ) == 1
    assert kotlin.count("assertTrue(\n            available,") == 2
    assert java.count("if (!available) {") == 2
    assert java.count("throw new AssertionError(") >= 2


def test_privacy_jvm_gate_builds_and_authenticates_native_abi22() -> None:
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


def test_javascript_lane_builds_and_executes_real_napi_abi22() -> None:
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
    assert "not yet requalified" not in job
    assert "IROHA_PRIVACY_RELEASE_CARGO_LOCKFILE_PATH" in job
    assert '--lockfile-path "$IROHA_PRIVACY_RELEASE_CARGO_LOCKFILE_PATH"' in job
    assert "install -m 600" not in job
    assert "cargo fetch --locked" in job
    fetch_step = job[job.index(
        "- name: Prime privacy N-API dependencies from the frozen lock"
    ) : job.index("- name: Install JavaScript SDK dependencies")]
    assert 'RUSTC_BOOTSTRAP: "1"' in fetch_step
    assert "run: ci/check_privacy_js_sdk.sh" in job
    assert "Revalidate frozen JavaScript lock inputs" in job
    assert "if: always()" in job[job.index(
        "Revalidate frozen JavaScript lock inputs"
    ) :]
    for dependency in (
        "javascript/iroha_js/scripts/build-native.mjs",
        "javascript/iroha_js/scripts/build-dist.mjs",
        "javascript/iroha_js/scripts/copy-native.mjs",
        "javascript/iroha_js/scripts/native-build-provenance.mjs",
        "javascript/iroha_js/scripts/native-build-profile.mjs",
        "javascript/iroha_js/src/native.js",
        "javascript/iroha_js/src/nativeArtifactHash.js",
        "javascript/iroha_js/package.json",
        "javascript/iroha_js/package-lock.json",
        "javascript/iroha_js/test/nativeBuildProfile.test.js",
        "javascript/iroha_js/test/nativeBuildProvenance.test.js",
        "javascript/iroha_js/test/privacyExact12Network.test.js",
    ):
        assert f'- "{dependency}"' in workflow

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
    assert "PRIVACY_RELEASE_CARGO_LOCK_SEAL" in gate
    assert "assert_privacy_release_cargo_lock" in gate
    assert "privacy_sdk_assert_file_seal" in gate

    integration = read(
        "javascript/iroha_js/test/privacyNative.integration.test.js"
    )
    assert "getNativeBinding()" in integration
    assert "isPrivacyNativeAvailable(), true" in integration
    assert "globalThis.__IROHA_NATIVE_BINDING__, undefined" in integration
    assert "withNativeBinding" not in integration
    assert "privacyValidateCompiledProfileCatalogV1" in integration


def test_javascript_gate_rejects_late_external_lock_mutation(tmp_path: Path) -> None:
    root = tmp_path / "repo"
    ci = root / "ci"
    tools = tmp_path / "bin"
    js_root = root / "javascript/iroha_js"
    for directory in (ci, tools, js_root):
        directory.mkdir(parents=True)

    gate = ci / "check_privacy_js_sdk.sh"
    gate.write_text(read("ci/check_privacy_js_sdk.sh"), encoding="utf-8")
    shutil.copy2(ROOT / "ci/privacy_sdk_cargo_lockfile.sh", ci)
    tracked = root / "Cargo.lock"
    release = tmp_path / "Cargo.lock"
    tracked.write_text("tracked\n", encoding="utf-8")
    release.write_text("release\n", encoding="utf-8")

    fake_python = tools / "python"
    fake_python.write_text(
        "#!/usr/bin/env bash\n"
        "last=${!#}\n"
        "if [[ \" $* \" == *\" -S \"* ]]; then\n"
        f'  [[ "$last" == "{tracked}" ]] && echo "{TRACKED_ROOT_LOCK_SHA256}" || echo "{FROZEN_LOCK_SHA256}"\n'
        "elif [[ \"$last\" == \"$PRIVACY_TEST_TRACKED_LOCK\" ]]; then\n"
        "  echo tracked-seal\n"
        "elif grep -qx release \"$last\"; then\n"
        "  echo release-seal\n"
        "else\n"
        "  echo changed-release-seal\n"
        "fi\n",
        encoding="utf-8",
    )
    fake_node = tools / "node"
    fake_node.write_text(
        "#!/usr/bin/env bash\n"
        "if [[ \"${1:-}\" == --version ]]; then echo v20.20.0; exit 0; fi\n"
        "if [[ \"${1:-}\" == --eval ]]; then printf darwin-arm64-node20; exit 0; fi\n"
        "if [[ \"${PRIVACY_TEST_MUTATE_RELEASE:-0}\" == 1 && "
        "\" $* \" == *\" test/privacyExact12Network.test.js \"* ]]; then\n"
        "  printf 'mutated\\n' >\"$PRIVACY_TEST_RELEASE_LOCK\"\n"
        "fi\n"
        "exit 0\n",
        encoding="utf-8",
    )
    fake_rustup = tools / "rustup"
    fake_rustup.write_text(
        "#!/usr/bin/env bash\n"
        "case \"${!#}\" in\n"
        f'  cargo) echo "{tools / "cargo"}" ;;\n'
        f'  rustc) echo "{tools / "rustc"}" ;;\n'
        f'  rustdoc) echo "{tools / "rustdoc"}" ;;\n'
        "  *) exit 1 ;;\n"
        "esac\n",
        encoding="utf-8",
    )
    (tools / "cargo").write_text("#!/usr/bin/env bash\nexit 0\n", encoding="utf-8")
    (tools / "rustc").write_text(
        "#!/usr/bin/env bash\necho 'rustc 1.93.1 (fixture)'\n", encoding="utf-8"
    )
    (tools / "rustdoc").write_text("#!/usr/bin/env bash\nexit 0\n", encoding="utf-8")
    for executable in tools.iterdir():
        executable.chmod(0o700)

    environment = {
        **os.environ,
        "PRIVACY_JS_SDK_ROOT": str(root),
        "PRIVACY_JS_SDK_NODE_BIN": str(fake_node),
        "PRIVACY_JS_SDK_PYTHON_BIN": str(fake_python),
        "PRIVACY_JS_SDK_RUSTUP_BIN": str(fake_rustup),
        "IROHA_PRIVACY_RELEASE_CARGO_LOCKFILE_PATH": str(release),
        "PRIVACY_TEST_TRACKED_LOCK": str(tracked),
        "PRIVACY_TEST_RELEASE_LOCK": str(release),
    }
    accepted = subprocess.run(
        ["bash", str(gate)], env=environment, text=True, capture_output=True
    )
    assert accepted.returncode == 0, accepted.stderr

    rejected = subprocess.run(
        ["bash", str(gate)],
        env={**environment, "PRIVACY_TEST_MUTATE_RELEASE": "1"},
        text=True,
        capture_output=True,
    )
    assert rejected.returncode == 1
    assert "privacy JavaScript external Cargo.lock changed" in rejected.stderr


def test_python_lane_authenticates_and_executes_real_pyo3_abi22() -> None:
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


def test_swift_lanes_produce_slices_then_assemble_external_native_abi22() -> None:
    workflow = read(".github/workflows/pr_privacy_sdk_guard.yml")
    slice_job = swift_slice_job(workflow)
    assembly_job = swift_job(workflow)
    assert "permissions:\n  contents: read" in workflow
    assert 'APPLE_PRIVACY_PRODUCTION_ENABLED: "false"' in workflow
    for job in (slice_job, assembly_job):
        assert "runs-on: macos-26" in job
        assert "timeout-minutes: 180" in job
        assert "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1" in job
        assert job.count("persist-credentials: false") == 1
        assert "actions/setup-python@ece7cb06caefa5fff74198d8649806c4678c61a1" in job
        assert job.count(
            "      DEVELOPER_DIR: /Applications/Xcode_26.6.app/Contents/Developer\n"
        ) == 1
        assert job.count(
            "      NORITO_BRIDGE_DEVELOPER_DIR: /Applications/Xcode_26.6.app/Contents/Developer\n"
        ) == 1
        assert job.count(
            "      NORITO_BRIDGE_SLICE_BUILD_ID: ${{ github.run_id }}.${{ github.run_attempt }}\n"
        ) == 1
        assert "Bind reviewed Apple privacy mode" in job
        assert 'case "${APPLE_PRIVACY_PRODUCTION_ENABLED:-}" in' in job
        assert 'build_args+=(--privacy-production-enabled)' in job
        assert "Xcode 26.6\\nBuild version 17F113" in job
        assert "unexpected DEVELOPER_DIR" in job
        assert "bridge and job Xcode identities differ" in job
        assert job.count("exit 1; }") == 4
        assert "dtolnay/rust-toolchain" not in job
        assert '"1.93.1-aarch64-apple-darwin"' in job
        assert "RUSTUP_TOOLCHAIN=1.93.1-aarch64-apple-darwin" in job
        assert "python3 -I -S" not in job
        assert "actions/download-artifact@" in job
        assert FROZEN_LOCK_SHA256 in job
        assert TRACKED_ROOT_LOCK_SHA256 in job
        assert "not yet requalified" not in job
        assert "IROHA_PRIVACY_RELEASE_CARGO_LOCKFILE_PATH" in job
        assert '--lockfile-path "$IROHA_PRIVACY_RELEASE_CARGO_LOCKFILE_PATH"' in job
        assert '[[ "$(git rev-parse HEAD)" == "$GITHUB_SHA" ]]' in job
        assert "install -m 600" not in job
        assert "NORITO_BRIDGE_OUT_DIR=" in job
        assert "NORITO_BRIDGE_BUILD_DIR=" in job
        assert "cargo fetch --locked" in job
        assert "chmod -R a-w" in job
        assert job.count("scripts/build_norito_xcframework.sh") == 1
        assert job.index("Require the exact Xcode 26.6 release toolchain") < job.index(
            "scripts/build_norito_xcframework.sh"
        )

    assert "needs: privacy_jvm_sdk_tests" in slice_job
    assert "max-parallel: 5" in slice_job
    assert slice_job.count("          - ios-") == 3
    assert slice_job.count("          - macos-") == 2
    assert '--produce-slice "${{ matrix.slice }}"' in slice_job
    assert "--assemble-slices" not in slice_job
    assert "actions/upload-artifact@" in slice_job
    assert (
        "privacy-swift-apple-slice-${{ github.run_id }}-${{ github.run_attempt }}-${{ matrix.slice }}"
        in slice_job
    )
    for job in (slice_job, assembly_job):
        assert "nohup" not in job
        assert re.search(r"(?<![>&])&(?![>&])", job) is None

    assert "needs: privacy_swift_sdk_slice" in assembly_job
    assert "MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT=1" in assembly_job
    assert '--assemble-slices "$NORITO_BRIDGE_SLICE_INPUT_ROOT"' in assembly_job
    assert "--produce-slice" not in assembly_job
    assert assembly_job.count("actions/download-artifact@") == 6
    for slice_id in (
        "ios-arm64",
        "ios-sim-arm64",
        "ios-sim-x64",
        "macos-arm64",
        "macos-x64",
    ):
        assert (
            f"privacy-swift-apple-slice-${{{{ github.run_id }}}}-${{{{ github.run_attempt }}}}-{slice_id}"
            in assembly_job
        )
        assert f"iroha-privacy-swift-slices/{slice_id}" in assembly_job
    assert "run: ci/check_privacy_swift_sdk.sh" in assembly_job
    assert "Build authoritative Offline Cash Swift fixture" in assembly_job
    assert "Revalidate frozen Swift inputs and ABI22 artifacts" in assembly_job
    assert assembly_job.count("scripts/check_mobile_sdk_artifacts.sh --apple-only") == 1
    background_mutation = assembly_job.replace(
        '            "${build_args[@]}"\n',
        '            "${build_args[@]}" & wait\n',
        1,
    )
    assert background_mutation != assembly_job
    assert re.search(r"(?<![>&])&(?![>&])", background_mutation) is not None

    gate = read("ci/check_privacy_swift_sdk.sh")
    assert f'FROZEN_CARGO_LOCK_SHA256="{FROZEN_LOCK_SHA256}"' in gate
    assert 'MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT:-}" != "1"' in gate
    assert "must remain outside the source tree" in gate
    assert 'if [[ -z "${DEVELOPER_DIR:-}" ]]; then' in gate
    assert 'DEVELOPER_DIR="$(xcode-select -p)"' in gate
    assert "does not match the authenticated Apple artifact toolchain" in gate
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
