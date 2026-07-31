"""Contract tests for host native SDK ABI-21 artifact evidence."""

from __future__ import annotations

import json
import os
import re
import subprocess
from pathlib import Path

import pytest

from scripts import check_native_sdk_abi21_artifact as checker
from scripts import run_mobile_hermetic_command as hermetic_runner


REPO_ROOT = Path(__file__).resolve().parents[2]

NATIVE_ESCROW_SHARED_TRIGGER_PATHS = {
    "Cargo.lock",
    "Cargo.toml",
    "crates/connect_norito_bridge/**",
    "crates/iroha_core/src/smartcontracts/isi/escrow.rs",
    "crates/iroha_core/src/smartcontracts/isi/mod.rs",
    "crates/iroha_data_model/src/bin/cancel_asset_lock_fixtures.rs",
    "crates/iroha_data_model/src/escrow.rs",
    "crates/iroha_data_model/src/events/data/escrow.rs",
    "crates/iroha_data_model/src/isi/escrow.rs",
    "crates/iroha_data_model/src/isi/mod.rs",
    "crates/iroha_data_model/src/isi/registry.rs",
    "crates/iroha_data_model/src/testing/cancel_asset_lock.rs",
    "crates/kotodama_lang/src/samples/native_escrow.ko",
    "crates/kotodama_lang/src/samples/native_escrow.to",
    "crates/sorafs_manifest/**",
    "fixtures/sorafs_manifest/appeal_finance/**",
    "fixtures/sorafs_manifest/reference_sdk/**",
    "fixtures/sorafs_manifest/reference_sdk_validation_inventory_v1.json",
    "integration_tests/tests/native_escrow.rs",
    "rust-toolchain.toml",
    "scripts/check_native_sdk_abi21_artifact.py",
    "scripts/check_sorafs_reference_sdk_fixtures.py",
    "scripts/tests/check_native_sdk_abi21_artifact_test.py",
    "scripts/tests/check_sorafs_fixture_workflow_contract_test.py",
}
NATIVE_ESCROW_WORKFLOW_SPECIFIC_TRIGGER_PATHS = {
    "pr_csharp.yml": {
        ".github/workflows/pr_csharp.yml",
        "ci/check_csharp_sdk_package_consumer.sh",
        "csharp/**",
        "scripts/package_csharp_native_artifacts.py",
        "scripts/tests/package_csharp_native_artifacts_test.py",
    },
    "mobile_sdk_artifacts.yml": {
        ".github/workflows/mobile_sdk_artifacts.yml",
        "IrohaSwift/**",
        "ci/check_kagemusha_jvm_native_bridge.sh",
        "java/iroha_android/**",
        "kotlin/**",
        "scripts/build_norito_xcframework.sh",
        "scripts/check_mobile_sdk_artifact_pin_commit.py",
        "scripts/check_mobile_sdk_artifacts.sh",
        "scripts/check_mobile_sdk_artifacts_test.sh",
        "scripts/exec_with_file_lock.py",
        "scripts/norito_bridge_source_seal.py",
        "scripts/package_mobile_sdk_artifacts.sh",
        "scripts/run_mobile_hermetic_command.py",
        "scripts/tests/mobile_sdk_python312_contract.sh",
    },
    "sorafs-orchestrator-sdk.yml": {
        ".github/workflows/sorafs-orchestrator-sdk.yml",
        "IrohaSwift/**",
        "ci/check_sorafs_python_native_sdk.sh",
        "ci/sdk_sorafs_orchestrator.sh",
        "crates/iroha_js_host/**",
        "javascript/iroha_js/**",
        "python/iroha_python/**",
        "scripts/build_norito_xcframework.sh",
        "scripts/check_mobile_sdk_artifact_pin_commit.py",
        "scripts/check_mobile_sdk_artifacts.sh",
        "scripts/exec_with_file_lock.py",
        "scripts/norito_bridge_source_seal.py",
        "scripts/run_mobile_hermetic_command.py",
    },
}


def pull_request_paths(workflow_name: str) -> set[str]:
    """Return one workflow's literal pull-request path contract."""

    source = (
        REPO_ROOT / ".github" / "workflows" / workflow_name
    ).read_text(encoding="utf-8")
    pull_request = re.search(
        r"(?ms)^  pull_request:\n(?P<body>(?:^    .*\n)*)",
        source,
    )
    assert pull_request is not None, f"{workflow_name} must define pull_request"
    paths = re.search(
        r"(?ms)^    paths:\n(?P<paths>(?:^      - .*\n)+)",
        pull_request.group("body"),
    )
    assert paths is not None, f"{workflow_name} must define pull_request.paths"
    return {
        line.removeprefix("      - ").strip().strip('"')
        for line in paths.group("paths").splitlines()
    }


def git(root: Path, *arguments: str) -> None:
    """Run one isolated Git command for a temporary source fixture."""

    environment = os.environ.copy()
    environment["GIT_CONFIG_GLOBAL"] = os.devnull
    environment["GIT_CONFIG_NOSYSTEM"] = "1"
    subprocess.run(
        ["git", "-C", str(root), *arguments],
        check=True,
        capture_output=True,
        env=environment,
    )


def clean_source(tmp_path: Path) -> Path:
    """Create one clean source repository."""

    source = tmp_path / "source"
    source.mkdir()
    (source / "tracked.txt").write_text("reviewed source\n", encoding="utf-8")
    git(source, "init", "-q")
    git(source, "config", "user.name", "ABI Contract Test")
    git(source, "config", "user.email", "abi-contract@example.invalid")
    git(source, "add", "tracked.txt")
    git(source, "commit", "-q", "-m", "fixture")
    return source


def native_artifact(tmp_path: Path) -> Path:
    """Create one non-empty artifact fixture outside the source tree."""

    artifact = tmp_path / "native.bin"
    artifact.write_bytes(b"native artifact bytes")
    return artifact


def exact_probe(_sdk: str, _path: Path) -> int:
    """Return the sole accepted first-release bridge ABI."""

    return checker.REQUIRED_BRIDGE_ABI_VERSION


@pytest.mark.parametrize(
    ("workflow_name", "workflow_specific"),
    NATIVE_ESCROW_WORKFLOW_SPECIFIC_TRIGGER_PATHS.items(),
)
def test_native_sdk_workflow_triggers_are_closed_over_native_escrow_inputs(
    workflow_name: str,
    workflow_specific: set[str],
) -> None:
    """Fixture, source, manifest, and artifact changes must rerun native parity."""

    actual = pull_request_paths(workflow_name)
    required = NATIVE_ESCROW_SHARED_TRIGGER_PATHS | workflow_specific
    missing = sorted(required - actual)
    assert not missing, (
        f"{workflow_name} omits native-escrow or appeal-finance triggers: {missing}"
    )


def test_record_and_verify_bind_exact_artifact_and_clean_revision(
    tmp_path: Path,
) -> None:
    source = clean_source(tmp_path)
    artifact = native_artifact(tmp_path)
    manifest = checker.build_manifest(
        sdk="csharp",
        target="x86_64-unknown-linux-gnu",
        artifact_path=artifact,
        source_root=source,
        probe=exact_probe,
    )

    assert manifest["bridge_abi_version"] == 21
    assert manifest["source_tree_clean"] is True
    assert manifest["required_symbols"] == list(checker.REQUIRED_SYMBOLS["csharp"])
    checker.verify_manifest(
        manifest,
        artifact_path=artifact,
        source_root=source,
        probe=exact_probe,
    )


def test_record_and_verify_reject_missing_artifact(tmp_path: Path) -> None:
    source = clean_source(tmp_path)
    missing = tmp_path / "missing-native.bin"

    with pytest.raises(checker.ArtifactContractError, match="unavailable"):
        checker.build_manifest(
            sdk="python",
            target="aarch64-apple-darwin",
            artifact_path=missing,
            source_root=source,
            probe=exact_probe,
        )

    artifact = native_artifact(tmp_path)
    manifest = checker.build_manifest(
        sdk="python",
        target="aarch64-apple-darwin",
        artifact_path=artifact,
        source_root=source,
        probe=exact_probe,
    )
    artifact.unlink()
    with pytest.raises(checker.ArtifactContractError, match="unavailable"):
        checker.verify_manifest(
            manifest,
            artifact_path=artifact,
            source_root=source,
            probe=exact_probe,
        )


@pytest.mark.parametrize("observed", [19, 20, 22])
def test_record_rejects_every_non_exact_abi(
    tmp_path: Path,
    observed: int,
) -> None:
    source = clean_source(tmp_path)
    artifact = native_artifact(tmp_path)

    with pytest.raises(checker.ArtifactContractError, match="exactly 21"):
        checker.build_manifest(
            sdk="c-jni",
            target="aarch64-apple-darwin",
            artifact_path=artifact,
            source_root=source,
            probe=lambda _sdk, _path: observed,
        )


@pytest.mark.parametrize("observed", [20, 22])
def test_verify_rejects_every_non_exact_abi(
    tmp_path: Path,
    observed: int,
) -> None:
    source = clean_source(tmp_path)
    artifact = native_artifact(tmp_path)
    manifest = checker.build_manifest(
        sdk="csharp",
        target="x86_64-unknown-linux-gnu",
        artifact_path=artifact,
        source_root=source,
        probe=exact_probe,
    )

    with pytest.raises(checker.ArtifactContractError, match="exactly 21"):
        checker.verify_manifest(
            manifest,
            artifact_path=artifact,
            source_root=source,
            probe=lambda _sdk, _path: observed,
        )


def test_record_and_verify_reject_dirty_or_stale_source(
    tmp_path: Path,
) -> None:
    source = clean_source(tmp_path)
    artifact = native_artifact(tmp_path)
    manifest = checker.build_manifest(
        sdk="python",
        target="aarch64-apple-darwin",
        artifact_path=artifact,
        source_root=source,
        probe=exact_probe,
    )

    (source / "untracked.py").write_text("dirty = True\n", encoding="utf-8")
    with pytest.raises(checker.ArtifactContractError, match="clean source tree"):
        checker.build_manifest(
            sdk="python",
            target="aarch64-apple-darwin",
            artifact_path=artifact,
            source_root=source,
            probe=exact_probe,
        )
    with pytest.raises(checker.ArtifactContractError, match="current clean source revision"):
        checker.verify_manifest(
            manifest,
            artifact_path=artifact,
            source_root=source,
            probe=exact_probe,
        )


def test_verify_rejects_replaced_artifact(
    tmp_path: Path,
) -> None:
    source = clean_source(tmp_path)
    artifact = native_artifact(tmp_path)
    manifest = checker.build_manifest(
        sdk="node",
        target="aarch64-apple-darwin",
        artifact_path=artifact,
        source_root=source,
        probe=exact_probe,
    )
    artifact.write_bytes(b"different artifact bytes")

    with pytest.raises(checker.ArtifactContractError, match="artifact bytes"):
        checker.verify_manifest(
            manifest,
            artifact_path=artifact,
            source_root=source,
            probe=exact_probe,
        )


def test_record_rejects_artifact_replaced_during_probe(tmp_path: Path) -> None:
    source = clean_source(tmp_path)
    artifact = native_artifact(tmp_path)

    def replacing_probe(_sdk: str, path: Path) -> int:
        path.write_bytes(b"replacement with the same ABI")
        return checker.REQUIRED_BRIDGE_ABI_VERSION

    with pytest.raises(checker.ArtifactContractError, match="changed while"):
        checker.build_manifest(
            sdk="node",
            target="aarch64-apple-darwin",
            artifact_path=artifact,
            source_root=source,
            probe=replacing_probe,
        )


def test_cli_rejects_final_component_artifact_symlink(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    source = clean_source(tmp_path)
    artifact = native_artifact(tmp_path)
    symlink = tmp_path / "native-link.bin"
    symlink.symlink_to(artifact)
    monkeypatch.setattr(
        "sys.argv",
        [
            "check_native_sdk_abi21_artifact.py",
            "record",
            "--artifact",
            str(symlink),
            "--manifest",
            str(tmp_path / "manifest.json"),
            "--source-root",
            str(source),
            "--sdk",
            "csharp",
            "--target",
            "aarch64-apple-darwin",
        ],
    )

    with pytest.raises(checker.ArtifactContractError, match="non-empty regular file"):
        checker.main()


def test_manifest_loader_rejects_noncanonical_and_duplicate_json(
    tmp_path: Path,
) -> None:
    source = clean_source(tmp_path)
    artifact = native_artifact(tmp_path)
    manifest = checker.build_manifest(
        sdk="node",
        target="linux-x64",
        artifact_path=artifact,
        source_root=source,
        probe=exact_probe,
    )
    path = tmp_path / "manifest.json"
    path.write_bytes(checker.canonical_manifest_bytes(manifest))
    assert checker.load_manifest(path) == manifest

    path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    with pytest.raises(checker.ArtifactContractError, match="not canonical"):
        checker.load_manifest(path)

    path.write_text(
        '{"schema":"x","schema":"y"}\n',
        encoding="utf-8",
    )
    with pytest.raises(checker.ArtifactContractError, match="duplicate key"):
        checker.load_manifest(path)


def test_manifest_loader_rejects_symlink_and_hardlink(
    tmp_path: Path,
) -> None:
    source = clean_source(tmp_path)
    artifact = native_artifact(tmp_path)
    manifest = checker.build_manifest(
        sdk="node",
        target="linux-x64",
        artifact_path=artifact,
        source_root=source,
        probe=exact_probe,
    )
    path = tmp_path / "manifest.json"
    path.write_bytes(checker.canonical_manifest_bytes(manifest))

    symlink = tmp_path / "manifest-link.json"
    symlink.symlink_to(path)
    with pytest.raises(checker.ArtifactContractError, match="one hard link"):
        checker.load_manifest(symlink)

    hardlink = tmp_path / "manifest-hardlink.json"
    os.link(path, hardlink)
    with pytest.raises(checker.ArtifactContractError, match="one hard link"):
        checker.load_manifest(path)


def test_manifest_loader_rejects_final_path_replacement(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    source = clean_source(tmp_path)
    artifact = native_artifact(tmp_path)
    manifest = checker.build_manifest(
        sdk="node",
        target="linux-x64",
        artifact_path=artifact,
        source_root=source,
        probe=exact_probe,
    )
    path = tmp_path / "manifest.json"
    payload = checker.canonical_manifest_bytes(manifest)
    path.write_bytes(payload)
    replacement = tmp_path / "replacement.json"
    replacement.write_bytes(payload)

    real_read = os.read
    replaced = False

    def replace_during_read(descriptor: int, count: int) -> bytes:
        nonlocal replaced
        if not replaced:
            replaced = True
            os.replace(replacement, path)
        return real_read(descriptor, count)

    monkeypatch.setattr(checker.os, "read", replace_during_read)
    with pytest.raises(checker.ArtifactContractError, match="changed while it was read"):
        checker.load_manifest(path)


def test_node_probe_requires_exports_and_exact_integer_abi(
    tmp_path: Path,
) -> None:
    complete = tmp_path / "complete.cjs"
    complete.write_text(
        "module.exports = {"
        "connectNoritoBridgeAbiVersion() { return 21; },"
        "sorafsValidateAppealFinanceCancelAssetLockJson() {}"
        "};\n",
        encoding="utf-8",
    )
    assert (
        checker.probe_node_abi(
            complete,
            checker.REQUIRED_SYMBOLS["node"],
        )
        == 21
    )

    complete.write_text(
        "module.exports = {connectNoritoBridgeAbiVersion() { return 19; }};\n",
        encoding="utf-8",
    )
    with pytest.raises(checker.ArtifactContractError, match="missing required exports"):
        checker.probe_node_abi(complete, checker.REQUIRED_SYMBOLS["node"])


def test_python_probe_requires_exports_and_exact_integer_abi(
    tmp_path: Path,
) -> None:
    complete = tmp_path / "complete.py"
    complete.write_text(
        "def connect_norito_bridge_abi_version():\n"
        "    return 21\n"
        "def sorafs_validate_appeal_finance_cancel_asset_lock_json():\n"
        "    return '{}'\n",
        encoding="utf-8",
    )
    assert (
        checker.probe_python_abi(
            complete,
            checker.REQUIRED_SYMBOLS["python"],
        )
        == 21
    )

    complete.write_text(
        "def connect_norito_bridge_abi_version():\n"
        "    return '21'\n"
        "def sorafs_validate_appeal_finance_cancel_asset_lock_json():\n"
        "    return '{}'\n",
        encoding="utf-8",
    )
    with pytest.raises(checker.ArtifactContractError, match="ABI probe returned"):
        checker.probe_python_abi(complete, checker.REQUIRED_SYMBOLS["python"])


def test_repository_wires_exact_abi21_release_contract() -> None:
    """Freeze the fail-closed source and CI wiring without loading a native binary."""

    def read(relative: str) -> str:
        return (REPO_ROOT / relative).read_text(encoding="utf-8")

    node_copy = read("javascript/iroha_js/scripts/copy-native.mjs")
    for token in (
        'export const REQUIRED_NATIVE_BRIDGE_ABI_VERSION = 21;',
        '"connectNoritoBridgeAbiVersion"',
        '"sorafsValidateAppealFinanceCancelAssetLockJson"',
        "buildProvenance.source_tree_clean !== true",
    ):
        assert token in node_copy

    python_native = read("python/iroha_python/iroha_python_rs/src/lib.rs")
    assert '#[pyo3(name = "connect_norito_bridge_abi_version")]' in python_native
    assert "fn connect_norito_bridge_abi_version_py() -> u32" in python_native
    assert "connect_norito_bridge_abi_version_py," in python_native

    csharp = read(
        "csharp/src/Hyperledger.Iroha.Sdk/SoraFs/SoraFsReferenceValidators.cs"
    )
    assert csharp.count("native.AbiVersion() == RequiredBridgeAbiVersion") == 4
    assert "version != RequiredBridgeAbiVersion" in csharp
    assert "native.AbiVersion() >= RequiredBridgeAbiVersion" not in csharp

    kotlin = read(
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/sorafs/"
        "SorafsReferenceValidators.kt"
    )
    java = read(
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/sorafs/"
        "SorafsReferenceValidators.java"
    )
    assert "version == REQUIRED_BRIDGE_ABI_VERSION" in kotlin
    assert "abiVersion == REQUIRED_BRIDGE_ABI_VERSION" in java
    kotlin_signer = read(
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/crypto/"
        "NativeSignerBridge.kt"
    )
    java_signer = read(
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/crypto/"
        "NativeSignerBridge.java"
    )
    assert "nativeBridgeAbiVersion() == REQUIRED_BRIDGE_ABI_VERSION" in kotlin_signer
    assert "nativeBridgeAbiVersion() == REQUIRED_BRIDGE_ABI_VERSION" in java_signer
    assert (
        "nativeSignerContractRevision() == REQUIRED_NATIVE_SIGNER_CONTRACT_REVISION"
        in kotlin_signer
    )
    assert (
        "nativeSignerContractRevision() == REQUIRED_NATIVE_SIGNER_CONTRACT_REVISION"
        in java_signer
    )
    assert "REQUIRED_NATIVE_SIGNER_CONTRACT_REVISION: Int = 1" in kotlin_signer
    assert "REQUIRED_NATIVE_SIGNER_CONTRACT_REVISION = 1" in java_signer
    assert "nativeBridgeAbiVersion() >= REQUIRED_BRIDGE_ABI_VERSION" not in kotlin_signer
    assert "nativeBridgeAbiVersion() >= REQUIRED_BRIDGE_ABI_VERSION" not in java_signer
    assert (
        "nativeSignerContractRevision() >= REQUIRED_NATIVE_SIGNER_CONTRACT_REVISION"
        not in kotlin_signer
    )
    assert (
        "nativeSignerContractRevision() >= REQUIRED_NATIVE_SIGNER_CONTRACT_REVISION"
        not in java_signer
    )
    kotlin_tests = read(
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/sorafs/"
        "SorafsReferenceValidatorsTest.kt"
    )
    java_tests = read(
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/sorafs/"
        "SorafsReferenceValidatorsTests.java"
    )
    assert "assumeTrue(false" not in kotlin_tests
    assert "IROHA_REQUIRE_SORAFS_NATIVE_VALIDATION" not in kotlin_tests
    assert "throw AssertionError(requiredMessage)" in kotlin_tests
    assert "IROHA_REQUIRE_SORAFS_NATIVE_VALIDATION" not in java_tests
    assert (
        "ABI-21 connect_norito_bridge with all SoraFS reference symbols is required."
        in java_tests
    )

    swift = read("IrohaSwift/Sources/IrohaSwift/NativeBridge.swift")
    assert "return actual == expectedBridgeAbiVersion(for: identifier)" in swift
    swift_tests = read(
        "IrohaSwift/Tests/IrohaSwiftTests/SorafsReferenceValidatorsTests.swift"
    )
    assert "throw XCTSkip(unavailableMessage)" not in swift_tests
    assert "IROHA_REQUIRE_SORAFS_NATIVE_VALIDATION" not in swift_tests
    assert (
        'XCTFail("\\(Self.nativeValidationRequiredMessage) \\(unavailableMessage)")'
        in swift_tests
    )

    def read_test_tree(relative: str, suffix: str) -> str:
        root = REPO_ROOT / relative
        return "\n".join(
            path.read_text(encoding="utf-8")
            for path in sorted(root.rglob(f"*{suffix}"))
            if path.is_file()
        )

    all_swift_tests = read_test_tree("IrohaSwift/Tests", ".swift")
    assert "XCTSkipIf(" not in all_swift_tests
    assert "XCTSkipUnless(" not in all_swift_tests
    native_swift_skip = re.compile(
        r"throw\s+XCTSkip\(\"[^\"\n]*"
        r"(?:NoritoNativeBridge|NoritoBridge|SorafsReferenceValidators|"
        r"PrivacyNativeBridge|KagemushaRecursiveSpend|native bridge|"
        r"native encoder|ABI-21 bridge|bridge functions)",
    )
    assert native_swift_skip.search(all_swift_tests) is None
    assert "func requireNativeTestCapability(" in all_swift_tests
    assert "RequiredNativeTestCapabilityError.unavailable" in all_swift_tests

    all_kotlin_tests = read_test_tree("kotlin/core-jvm/src/test", ".kt")
    assert "assumeTrue(" not in all_kotlin_tests
    assert "IROHA_REQUIRE_KAGEMUSHA_NATIVE" not in all_kotlin_tests
    assert "assertNativeArtifactStreamingUnavailableFailsClosed" not in all_kotlin_tests
    assert re.search(
        r"if\s*\(\s*!NativeSignerBridge\.isNativeAvailable\(\)\s*\)\s*return\b",
        all_kotlin_tests,
    ) is None
    assert (
        "A freshly built connect_norito_bridge ABI 21 "
        "artifact-streaming library is required"
        in all_kotlin_tests
    )

    all_java_tests = read_test_tree("java/iroha_android/src/test", ".java")
    assert "org.junit.Assume" not in all_java_tests
    assert "IROHA_REQUIRE_KAGEMUSHA_NATIVE" not in all_java_tests
    assert "Skipping ML-DSA" not in all_java_tests
    assert "assertNativeArtifactStreamingUnavailableFailsClosed" not in all_java_tests
    assert re.search(
        r"if\s*\(\s*!NativeSignerBridge\.isNativeAvailable\(\)\s*\)"
        r"\s*\{\s*return;",
        all_java_tests,
    ) is None
    assert (
        "A freshly built connect_norito_bridge ABI 21 "
        "artifact-streaming library is required"
        in all_java_tests
    )

    all_javascript_tests = "\n".join(
        (
            read_test_tree("javascript/iroha_js/test", ".js"),
            read_test_tree("javascript/iroha_js/test", ".mjs"),
            read_test_tree("javascript/iroha_js/test", ".ts"),
        )
    )
    assert re.search(
        r"\b(?:test|it|describe)\.skip\s*\(",
        all_javascript_tests,
    ) is None
    assert "test(\"compute simulation echoes payload by default\"" in all_javascript_tests

    csharp_workflow = read(".github/workflows/pr_csharp.yml")
    python_lane = read("ci/check_sorafs_python_native_sdk.sh")
    for lane in (csharp_workflow, python_lane):
        assert "check_native_sdk_abi21_artifact.py" in lane
        assert "record" in lane
        assert "verify" in lane
    assert 'PYTHON_VERSION}" != "3.12"' in python_lane
    assert "sys.version_info.major}{sys.version_info.minor}" in python_lane
    node_lane = read("ci/sdk_sorafs_orchestrator.sh")
    assert "native/iroha_js_host.node" in node_lane
    assert "--sdk node" in node_lane
    assert node_lane.count("check_native_sdk_abi21_artifact.py") == 2
    assert "record" in node_lane
    assert "verify" in node_lane

    mobile_checker = read("scripts/check_mobile_sdk_artifacts.sh")
    mobile_workflow = read(".github/workflows/mobile_sdk_artifacts.yml")
    assert '"native_bridge_abi_version"] != 21' in mobile_checker
    assert "check_mobile_sdk_artifacts.sh --apple-only" in mobile_workflow
    assert "check_kagemusha_jvm_native_bridge.sh" in mobile_workflow
    jni_lane = read("ci/check_kagemusha_jvm_native_bridge.sh")
    assert 'ABI21_ARTIFACT_CHECKER="$ROOT_DIR/scripts/check_native_sdk_abi21_artifact.py"' in jni_lane
    assert "resolve_trusted_python312()" in jni_lane
    assert "MOBILE_SDK_PYTHON_BINARY" in jni_lane
    assert "sys.version_info[:2] != (3, 12)" in jni_lane
    assert '"$PYTHON_BINARY" -I -S' in jni_lane
    assert '--set "IROHA_REQUIRE_SORAFS_NATIVE_VALIDATION=1"' in jni_lane
    assert (
        "IROHA_REQUIRE_SORAFS_NATIVE_VALIDATION"
        in hermetic_runner.PROFILES["gradle-jvm"]
    )
    assert "--sdk c-jni" in jni_lane
