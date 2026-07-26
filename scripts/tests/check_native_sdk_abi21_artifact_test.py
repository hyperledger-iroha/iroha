"""Contract tests for host native SDK ABI-21 artifact evidence."""

from __future__ import annotations

import json
import os
import subprocess
from pathlib import Path

import pytest

from scripts import check_native_sdk_abi21_artifact as checker


REPO_ROOT = Path(__file__).resolve().parents[2]


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
        "sourceTreeClean !== true",
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
    kotlin_tests = read(
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/sorafs/"
        "SorafsReferenceValidatorsTest.kt"
    )
    java_tests = read(
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/sorafs/"
        "SorafsReferenceValidatorsTests.java"
    )
    assert 'System.getenv("IROHA_REQUIRE_SORAFS_NATIVE_VALIDATION") == "1"' in kotlin_tests
    assert '"1".equals(System.getenv("IROHA_REQUIRE_SORAFS_NATIVE_VALIDATION"))' in java_tests

    swift = read("IrohaSwift/Sources/IrohaSwift/NativeBridge.swift")
    assert "return actual == expectedBridgeAbiVersion(for: identifier)" in swift

    csharp_workflow = read(".github/workflows/pr_csharp.yml")
    python_lane = read("ci/check_sorafs_python_native_sdk.sh")
    for lane in (csharp_workflow, python_lane):
        assert "check_native_sdk_abi21_artifact.py" in lane
        assert "record" in lane
        assert "verify" in lane
    assert 'PYTHON_VERSION}" != "3.12"' in python_lane
    assert "sys.version_info.major}{sys.version_info.minor}" in python_lane

    mobile_checker = read("scripts/check_mobile_sdk_artifacts.sh")
    mobile_workflow = read(".github/workflows/mobile_sdk_artifacts.yml")
    assert '"native_bridge_abi_version"] != 21' in mobile_checker
    assert "check_mobile_sdk_artifacts.sh --apple-only" in mobile_workflow
    assert "check_kagemusha_jvm_native_bridge.sh" in mobile_workflow
    jni_lane = read("ci/check_kagemusha_jvm_native_bridge.sh")
    assert 'ABI21_ARTIFACT_CHECKER="$ROOT_DIR/scripts/check_native_sdk_abi21_artifact.py"' in jni_lane
    assert '--set "IROHA_REQUIRE_SORAFS_NATIVE_VALIDATION=1"' in jni_lane
    assert "--sdk c-jni" in jni_lane
