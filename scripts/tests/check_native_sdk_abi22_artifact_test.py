"""Contract tests for host native SDK ABI-23 artifact evidence."""

from __future__ import annotations

import json
import os
import re
import stat
import subprocess
import sys
from pathlib import Path

import pytest

from scripts import check_native_sdk_abi22_artifact as checker


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
    "scripts/check_native_sdk_abi22_artifact.py",
    "scripts/compute_workspace_source_manifest.py",
    "scripts/check_sorafs_reference_sdk_fixtures.py",
    "scripts/tests/check_native_sdk_abi22_artifact_test.py",
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
        "crates/iroha_cli/**",
        "crates/iroha_config/**",
        "crates/iroha_core/**",
        "crates/iroha_data_model/**",
        "crates/iroha_kagami/**",
        "crates/iroha_torii/**",
        "crates/irohad/**",
        "java/iroha_android/**",
        "kotlin/**",
        "scripts/build_norito_xcframework.sh",
        "scripts/check_mobile_sdk_artifact_pin_commit.py",
        "scripts/check_mobile_sdk_artifacts.sh",
        "scripts/check_mobile_sdk_artifacts_test.sh",
        "scripts/exec_with_file_lock.py",
        "scripts/norito_bridge_source_seal.py",
        "scripts/package_mobile_sdk_artifacts.sh",
        "scripts/render_norito_bridge_podspec.py",
        "scripts/deploy_localnet.sh",
        "scripts/run_mobile_hermetic_command.py",
        "scripts/tests/deploy_localnet_test.py",
        "scripts/tests/mobile_sdk_python312_contract.sh",
        "scripts/tests/norito_bridge_source_seal_test.py",
        "scripts/tests/package_mobile_sdk_artifacts_test.py",
        "scripts/tests/render_norito_bridge_podspec_test.py",
    },
    "sorafs-orchestrator-sdk.yml": {
        ".cargo/**",
        ".github/workflows/sorafs-orchestrator-sdk.yml",
        ".github/workflows/mobile_sdk_artifacts.yml",
        ".github/workflows/pr_csharp.yml",
        "IrohaSwift/**",
        "ci/check_kagemusha_jvm_native_bridge.sh",
        "ci/check_sorafs_python_native_sdk.sh",
        "ci/sdk_sorafs_orchestrator.sh",
        "codec/**",
        "crates/iroha_cli/**",
        "crates/iroha_config/**",
        "crates/iroha_core/**",
        "crates/iroha_data_model/**",
        "crates/iroha_kagami/**",
        "crates/iroha_torii/**",
        "crates/irohad/**",
        "crates/iroha_js_host/**",
        "csharp/**",
        "gradle/mobile-sdk-external-android-build.settings.gradle.kts",
        "javascript/iroha_js/**",
        "java/iroha_android/**",
        "java/norito_java/**",
        "kotlin/**",
        "python/iroha_python/**",
        "scripts/build_norito_xcframework.sh",
        "scripts/check_mobile_sdk_artifact_pin_commit.py",
        "scripts/check_mobile_sdk_artifacts.sh",
        "scripts/check_sorafs_release_automation.py",
        "scripts/deploy_localnet.sh",
        "scripts/exec_with_file_lock.py",
        "scripts/norito_bridge_source_seal.py",
        "scripts/package_mobile_sdk_artifacts.sh",
        "scripts/run_mobile_hermetic_command.py",
        "scripts/tests/deploy_localnet_test.py",
        "scripts/tests/check_sorafs_release_automation_test.py",
        "scripts/tests/check_sorafs_python_native_sdk_evidence_contract.sh",
        "rust-toolchain",
        "vendor/**",
    },
}


def pull_request_paths(workflow_name: str) -> set[str]:
    """Return one workflow's literal pull-request path contract."""

    source = (
        REPO_ROOT / ".github" / "workflows" / workflow_name
    ).read_text(encoding="utf-8")
    pull_request = re.search(
        r"(?m)^  pull_request:\n(?P<body>(?:^    [^\n]*\n)*)",
        source,
    )
    assert pull_request is not None, f"{workflow_name} must define pull_request"
    paths = re.search(
        r'(?m)^    paths:\n(?P<paths>(?:^      - "[^"\n]+"\n)+)',
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


def exact_symbol_inventory(_path: Path) -> tuple[str, ...]:
    """Return the five approved privacy C exports for synthetic artifacts."""

    return checker.APPROVED_PRIVACY_C_EXPORTS


def test_direct_cli_loads_manifest_helper_under_isolated_python(tmp_path: Path) -> None:
    """The hermetic JVM lane must load the adjacent helper under ``-I -S``."""

    completed = subprocess.run(
        (
            sys.executable,
            "-I",
            "-S",
            "-B",
            str(REPO_ROOT / "scripts/check_native_sdk_abi22_artifact.py"),
            "--help",
        ),
        cwd=tmp_path,
        check=False,
        capture_output=True,
        text=True,
    )
    assert completed.returncode == 0, completed.stderr
    assert "{record,verify}" in completed.stdout


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
    assert not any(
        path.startswith(("jobs:", "name:", "run:", "runs-on:", "uses:"))
        for path in actual
    ), f"{workflow_name} path parser leaked workflow job fields"
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
        symbol_inventory=exact_symbol_inventory,
    )

    assert manifest["bridge_abi_version"] == 23
    assert manifest["source_tree_clean"] is True
    assert checker.SHA256_RE.fullmatch(
        str(manifest["workspace_source_manifest_sha256"])
    )
    assert manifest["privacy_c_exports_inspected"] is True
    assert manifest["privacy_c_exports"] == list(checker.APPROVED_PRIVACY_C_EXPORTS)
    assert manifest["required_symbols"] == list(checker.REQUIRED_SYMBOLS["csharp"])
    checker.verify_manifest(
        manifest,
        artifact_path=artifact,
        source_root=source,
        probe=exact_probe,
        symbol_inventory=exact_symbol_inventory,
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
            symbol_inventory=exact_symbol_inventory,
        )

    artifact = native_artifact(tmp_path)
    manifest = checker.build_manifest(
        sdk="python",
        target="aarch64-apple-darwin",
        artifact_path=artifact,
        source_root=source,
        probe=exact_probe,
        symbol_inventory=exact_symbol_inventory,
    )
    artifact.unlink()
    with pytest.raises(checker.ArtifactContractError, match="unavailable"):
        checker.verify_manifest(
            manifest,
            artifact_path=artifact,
            source_root=source,
            probe=exact_probe,
            symbol_inventory=exact_symbol_inventory,
        )


@pytest.mark.parametrize("observed", [19, 20, 21, 22])
def test_record_rejects_every_non_exact_abi(
    tmp_path: Path,
    observed: int,
) -> None:
    source = clean_source(tmp_path)
    artifact = native_artifact(tmp_path)

    with pytest.raises(checker.ArtifactContractError, match="exactly 23"):
        checker.build_manifest(
            sdk="c-jni",
            target="aarch64-apple-darwin",
            artifact_path=artifact,
            source_root=source,
            probe=lambda _sdk, _path: observed,
            symbol_inventory=exact_symbol_inventory,
        )


@pytest.mark.parametrize("observed", [20, 21, 22])
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
        symbol_inventory=exact_symbol_inventory,
    )

    with pytest.raises(checker.ArtifactContractError, match="exactly 23"):
        checker.verify_manifest(
            manifest,
            artifact_path=artifact,
            source_root=source,
            probe=lambda _sdk, _path: observed,
            symbol_inventory=exact_symbol_inventory,
        )


def test_bridge_requires_exact_five_privacy_c_exports(tmp_path: Path) -> None:
    source = clean_source(tmp_path)
    artifact = native_artifact(tmp_path)

    for missing in checker.APPROVED_PRIVACY_C_EXPORTS:
        inventory = tuple(
            symbol
            for symbol in checker.APPROVED_PRIVACY_C_EXPORTS
            if symbol != missing
        )
        with pytest.raises(checker.ArtifactContractError, match="missing approved"):
            checker.build_manifest(
                sdk="c-jni",
                target="aarch64-unknown-linux-gnu",
                artifact_path=artifact,
                source_root=source,
                probe=exact_probe,
                symbol_inventory=lambda _path, value=inventory: value,
            )


def test_privacy_c_export_inventory_rejects_duplicate_and_unexpected() -> None:
    approved = checker.APPROVED_PRIVACY_C_EXPORTS
    with pytest.raises(checker.ArtifactContractError, match="duplicate"):
        checker.validate_privacy_c_exports(
            (*approved, approved[0]),
            require_exact=True,
        )
    for unexpected in (
        "iroha_privacy_capabilities_v1",
        "iroha_privacy_proof_request_v1",
        "_iroha_privacy_compiled_profile_catalog_v1",
    ):
        with pytest.raises(checker.ArtifactContractError, match="privacy C symbol"):
            checker.validate_privacy_c_exports(
                (*approved, unexpected),
                require_exact=True,
            )


@pytest.mark.parametrize(
    "stale",
    (
        "iroha_privacy_compiled_profile_catalog_v21",
        "iroha_privacy_validate_compiled_profile_catalog_abi22",
        "connect_norito_privacy_abi_21_probe",
        "connect_norito_privacy_abi-22-probe",
        "connect_norito_bridge_abi_version_v21",
        "connect_norito_bridge_abi_version_v22",
    ),
)
def test_privacy_c_export_inventory_rejects_stale_abi_markers(stale: str) -> None:
    with pytest.raises(checker.ArtifactContractError, match="stale privacy/bridge ABI"):
        checker.validate_privacy_c_exports(
            (*checker.APPROVED_PRIVACY_C_EXPORTS, stale),
            require_exact=True,
        )


def test_node_and_python_do_not_invent_a_c_export_contract(tmp_path: Path) -> None:
    source = clean_source(tmp_path)
    artifact = native_artifact(tmp_path)
    manifest = checker.build_manifest(
        sdk="python",
        target="linux-x64-python312",
        artifact_path=artifact,
        source_root=source,
        probe=exact_probe,
        symbol_inventory=lambda _path: None,
    )

    assert manifest["privacy_c_exports_inspected"] is False
    assert manifest["privacy_c_exports"] == []
    checker.verify_manifest(
        manifest,
        artifact_path=artifact,
        source_root=source,
        probe=exact_probe,
        symbol_inventory=lambda _path: None,
    )


def test_verify_rejects_changed_or_missing_source_manifest_binding(
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
        symbol_inventory=exact_symbol_inventory,
    )

    changed = dict(manifest)
    changed["workspace_source_manifest_sha256"] = (
        "b" * 64
        if manifest["workspace_source_manifest_sha256"] != "b" * 64
        else "c" * 64
    )
    with pytest.raises(checker.ArtifactContractError, match="current source manifest"):
        checker.verify_manifest(
            changed,
            artifact_path=artifact,
            source_root=source,
            probe=exact_probe,
            symbol_inventory=exact_symbol_inventory,
        )

    missing = dict(manifest)
    del missing["workspace_source_manifest_sha256"]
    with pytest.raises(checker.ArtifactContractError, match="field inventory"):
        checker.verify_manifest(
            missing,
            artifact_path=artifact,
            source_root=source,
            probe=exact_probe,
            symbol_inventory=exact_symbol_inventory,
        )


def test_record_rejects_source_manifest_toctou(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    source = clean_source(tmp_path)
    artifact = native_artifact(tmp_path)
    original = checker.workspace_source_manifest_sha256(source)
    changed = "b" * 64 if original != "b" * 64 else "c" * 64
    observed = iter((original, changed))
    monkeypatch.setattr(
        checker,
        "workspace_source_manifest_sha256",
        lambda _root: next(observed),
    )

    with pytest.raises(checker.ArtifactContractError, match="source changed"):
        checker.build_manifest(
            sdk="csharp",
            target="x86_64-unknown-linux-gnu",
            artifact_path=artifact,
            source_root=source,
            probe=exact_probe,
            symbol_inventory=exact_symbol_inventory,
        )


def test_symbol_tool_parsers_preserve_duplicates_and_normalize_only_macho() -> None:
    symbol = checker.APPROVED_PRIVACY_C_EXPORTS[0]
    assert checker._parse_symbol_tool_output(
        f"_{symbol}\n_{symbol}\n".encode("ascii"),
        "macho-lines",
    ) == (symbol, symbol)
    assert checker._parse_symbol_tool_output(
        f"    1    0 0000000000001000 {symbol}\n".encode("ascii"),
        "dumpbin",
    ) == (symbol,)


def test_missing_symbol_tool_fails_closed_only_for_bridge_lanes(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    artifact = native_artifact(tmp_path)
    monkeypatch.setattr(checker.shutil, "which", lambda _tool: None)

    with pytest.raises(checker.ArtifactContractError, match="no supported symbol tool"):
        checker.inspect_exported_symbols(artifact, required=True)
    assert checker.inspect_exported_symbols(artifact, required=False) is None


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
        symbol_inventory=exact_symbol_inventory,
    )

    (source / "untracked.py").write_text("dirty = True\n", encoding="utf-8")
    with pytest.raises(checker.ArtifactContractError, match="clean source tree"):
        checker.build_manifest(
            sdk="python",
            target="aarch64-apple-darwin",
            artifact_path=artifact,
            source_root=source,
            probe=exact_probe,
            symbol_inventory=exact_symbol_inventory,
        )
    with pytest.raises(checker.ArtifactContractError, match="current clean source revision"):
        checker.verify_manifest(
            manifest,
            artifact_path=artifact,
            source_root=source,
            probe=exact_probe,
            symbol_inventory=exact_symbol_inventory,
        )


def test_release_job_git_config_keeps_windows_checkout_clean(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """The release-job override must defeat a CRLFAlways runner policy."""

    source = clean_source(tmp_path)
    tracked = source / "tracked.txt"
    tracked.unlink()
    global_config = tmp_path / "windows-system-gitconfig"
    subprocess.run(
        [
            "git",
            "config",
            "--file",
            str(global_config),
            "core.autocrlf",
            "true",
        ],
        check=True,
        capture_output=True,
    )
    checkout_environment = os.environ.copy()
    checkout_environment.update(
        {
            "GIT_CONFIG_GLOBAL": str(global_config),
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_CONFIG_COUNT": "1",
            "GIT_CONFIG_KEY_0": "core.autocrlf",
            "GIT_CONFIG_VALUE_0": "false",
        }
    )
    subprocess.run(
        ["git", "-C", str(source), "checkout-index", "--force", "--", "tracked.txt"],
        check=True,
        capture_output=True,
        env=checkout_environment,
    )
    assert tracked.read_bytes() == b"reviewed source\n"

    for name in ("GIT_CONFIG_COUNT", "GIT_CONFIG_KEY_0", "GIT_CONFIG_VALUE_0"):
        monkeypatch.setenv(name, checkout_environment[name])
    _commit, clean = checker.source_state(source)
    assert clean is True

    tracked.write_bytes(b"reviewed source\r\n")
    _commit, clean = checker.source_state(source)
    assert clean is False


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
        symbol_inventory=exact_symbol_inventory,
    )
    artifact.write_bytes(b"different artifact bytes")

    with pytest.raises(checker.ArtifactContractError, match="artifact bytes"):
        checker.verify_manifest(
            manifest,
            artifact_path=artifact,
            source_root=source,
            probe=exact_probe,
            symbol_inventory=exact_symbol_inventory,
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
            symbol_inventory=exact_symbol_inventory,
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
            "check_native_sdk_abi22_artifact.py",
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
        symbol_inventory=exact_symbol_inventory,
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
        symbol_inventory=exact_symbol_inventory,
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
        symbol_inventory=exact_symbol_inventory,
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


def test_retained_native_manifest_is_private_canonical_and_payload_free(
    tmp_path: Path,
) -> None:
    """Successful retention emits only the verified fixed-schema manifest."""

    source = clean_source(tmp_path)
    artifact = native_artifact(tmp_path)
    manifest = checker.build_manifest(
        sdk="python",
        target="darwin-arm64-python312",
        artifact_path=artifact,
        source_root=source,
        probe=exact_probe,
    )
    evidence_directory = tmp_path / "python-native-evidence"
    retained = checker.retain_verified_manifest(
        manifest,
        artifact_path=artifact,
        evidence_directory=evidence_directory,
        source_root=source,
        probe=exact_probe,
    )

    assert retained == evidence_directory / "python-native-abi22.json"
    assert checker.load_manifest(retained) == manifest
    assert {path.name for path in evidence_directory.iterdir()} == {
        "python-native-abi22.json"
    }
    directory_metadata = evidence_directory.lstat()
    manifest_metadata = retained.lstat()
    assert stat.S_ISDIR(directory_metadata.st_mode)
    assert stat.S_IMODE(directory_metadata.st_mode) == 0o700
    assert stat.S_ISREG(manifest_metadata.st_mode)
    assert manifest_metadata.st_nlink == 1
    assert stat.S_IMODE(manifest_metadata.st_mode) == 0o600
    retained_text = retained.read_text(encoding="ascii")
    assert str(tmp_path) not in retained_text
    assert "artifact_path" not in retained_text
    assert "private_key" not in retained_text


def test_verify_cli_retains_native_manifest_only_after_reauthentication(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """The shell-facing verify operation owns the opt-in retention action."""

    source = clean_source(tmp_path)
    artifact = native_artifact(tmp_path)
    manifest = checker.build_manifest(
        sdk="python",
        target="darwin-arm64-python312",
        artifact_path=artifact,
        source_root=source,
        probe=exact_probe,
    )
    manifest_path = tmp_path / "manifest.json"
    manifest_path.write_bytes(checker.canonical_manifest_bytes(manifest))
    evidence_directory = tmp_path / "retained"

    def cli_probe(
        _sdk: str,
        _path: Path,
        *,
        node: str,
        python: str,
    ) -> int:
        del node, python
        return checker.REQUIRED_BRIDGE_ABI_VERSION

    monkeypatch.setattr(checker, "probe_artifact", cli_probe)
    monkeypatch.setattr(
        "sys.argv",
        [
            "check_native_sdk_abi22_artifact.py",
            "verify",
            "--artifact",
            str(artifact),
            "--manifest",
            str(manifest_path),
            "--source-root",
            str(source),
            "--evidence-dir",
            str(evidence_directory),
        ],
    )

    assert checker.main() == 0
    assert checker.load_manifest(
        evidence_directory / "python-native-abi22.json"
    ) == manifest


def test_retained_native_manifest_is_not_created_when_reauthentication_fails(
    tmp_path: Path,
) -> None:
    """Stale artifact bytes fail before the evidence directory is created."""

    source = clean_source(tmp_path)
    artifact = native_artifact(tmp_path)
    manifest = checker.build_manifest(
        sdk="python",
        target="darwin-arm64-python312",
        artifact_path=artifact,
        source_root=source,
        probe=exact_probe,
    )
    artifact.write_bytes(b"stale native artifact")
    evidence_directory = tmp_path / "must-not-exist"

    with pytest.raises(checker.ArtifactContractError, match="artifact bytes"):
        checker.retain_verified_manifest(
            manifest,
            artifact_path=artifact,
            evidence_directory=evidence_directory,
            source_root=source,
            probe=exact_probe,
        )
    assert not evidence_directory.exists()


@pytest.mark.parametrize("relative", (Path("evidence"), Path("../evidence")))
def test_retained_native_manifest_rejects_relative_output(
    tmp_path: Path,
    relative: Path,
) -> None:
    """The opt-in evidence destination must be an explicit absolute path."""

    source = clean_source(tmp_path)
    artifact = native_artifact(tmp_path)
    manifest = checker.build_manifest(
        sdk="python",
        target="darwin-arm64-python312",
        artifact_path=artifact,
        source_root=source,
        probe=exact_probe,
    )

    with pytest.raises(checker.ArtifactContractError, match="bounded absolute"):
        checker.retain_verified_manifest(
            manifest,
            artifact_path=artifact,
            evidence_directory=relative,
            source_root=source,
            probe=exact_probe,
        )


def test_retained_native_manifest_rejects_existing_or_symlinked_output(
    tmp_path: Path,
) -> None:
    """Retention never merges with or follows a pre-existing leaf."""

    source = clean_source(tmp_path)
    artifact = native_artifact(tmp_path)
    manifest = checker.build_manifest(
        sdk="python",
        target="darwin-arm64-python312",
        artifact_path=artifact,
        source_root=source,
        probe=exact_probe,
    )
    existing = tmp_path / "existing"
    existing.mkdir()
    with pytest.raises(checker.ArtifactContractError, match="must be fresh"):
        checker.retain_verified_manifest(
            manifest,
            artifact_path=artifact,
            evidence_directory=existing,
            source_root=source,
            probe=exact_probe,
        )

    target = tmp_path / "target"
    target.mkdir()
    linked = tmp_path / "linked"
    linked.symlink_to(target, target_is_directory=True)
    with pytest.raises(checker.ArtifactContractError, match="must be fresh"):
        checker.retain_verified_manifest(
            manifest,
            artifact_path=artifact,
            evidence_directory=linked,
            source_root=source,
            probe=exact_probe,
        )


def test_retained_native_manifest_rejects_symlinked_ancestry(
    tmp_path: Path,
) -> None:
    """An alias in the requested output ancestry is rejected, not resolved."""

    source = clean_source(tmp_path)
    artifact = native_artifact(tmp_path)
    manifest = checker.build_manifest(
        sdk="python",
        target="darwin-arm64-python312",
        artifact_path=artifact,
        source_root=source,
        probe=exact_probe,
    )
    real_parent = tmp_path / "real-parent"
    real_parent.mkdir()
    linked_parent = tmp_path / "linked-parent"
    linked_parent.symlink_to(real_parent, target_is_directory=True)

    with pytest.raises(checker.ArtifactContractError, match="must not contain symlinks"):
        checker.retain_verified_manifest(
            manifest,
            artifact_path=artifact,
            evidence_directory=linked_parent / "evidence",
            source_root=source,
            probe=exact_probe,
        )


def test_retained_native_manifest_rejects_source_tree_destination(
    tmp_path: Path,
) -> None:
    """Evidence retention cannot invalidate the attested clean source tree."""

    source = clean_source(tmp_path)
    artifact = native_artifact(tmp_path)
    manifest = checker.build_manifest(
        sdk="python",
        target="darwin-arm64-python312",
        artifact_path=artifact,
        source_root=source,
        probe=exact_probe,
    )

    with pytest.raises(checker.ArtifactContractError, match="outside the source tree"):
        checker.retain_verified_manifest(
            manifest,
            artifact_path=artifact,
            evidence_directory=source / "evidence",
            source_root=source,
            probe=exact_probe,
        )


def test_python_native_evidence_retention_is_opt_in_and_uploaded() -> None:
    """Freeze the zero-skip, final-verify, and exact-file upload ordering."""

    runner = (REPO_ROOT / "ci/check_sorafs_python_native_sdk.sh").read_text(
        encoding="utf-8"
    )
    workflow = (
        REPO_ROOT / ".github/workflows/sorafs-orchestrator-sdk.yml"
    ).read_text(encoding="utf-8")

    assert 'SORAFS_PYTHON_SDK_EVIDENCE_DIR:-' in runner
    assert runner.count("--evidence-dir") == 1
    assert 'VERIFY_EVIDENCE_ARGS=()' in runner
    assert '"${VERIFY_EVIDENCE_ARGS[@]}"' in runner
    skip_audit = runner.index(
        "SoraFS native Python SDK parity may not contain skipped tests"
    )
    assert skip_audit < runner.index('VERIFY_EVIDENCE_ARGS=()')
    assert runner.index('VERIFY_EVIDENCE_ARGS=()') < runner.rindex("  verify \\")
    assert "SDK_SESSION}/pytest.xml" in runner
    assert "SDK_SESSION}/python-native-abi22.json" in runner

    evidence_directory = (
        "${{ runner.temp }}/iroha-sorafs-python-native-abi22-evidence"
    )
    evidence_file = f"{evidence_directory}/python-native-abi22.json"
    assert f"SORAFS_PYTHON_SDK_EVIDENCE_DIR: {evidence_directory}" in workflow
    assert "name: Upload verified Python ABI-23 evidence" in workflow
    assert evidence_file in workflow
    assert "actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02" in workflow
    upload = workflow.split("name: Upload verified Python ABI-23 evidence", 1)[1]
    upload = upload.split("- name: Upload parity evidence", 1)[0]
    assert "if: always()" in upload
    assert "if-no-files-found: error" in upload
    assert "retention-days: 30" in upload
    assert "pytest.xml" not in upload
    assert "iroha-sorafs-python-sdk" not in upload


def test_node_probe_requires_exports_and_exact_integer_abi(
    tmp_path: Path,
) -> None:
    complete = tmp_path / "complete.cjs"
    complete.write_text(
        "module.exports = {"
        "connectNoritoBridgeAbiVersion() { return 23; },"
        "inspectSorafsOrderbookSubmissionForDiscriminantV1() {},"
        "sorafsValidateAppealFinanceCancelAssetLockJson() {}"
        ",verifySorafsOrderbookSubmissionReceiptV1() {}"
        "};\n",
        encoding="utf-8",
    )
    assert (
        checker.probe_node_abi(
            complete,
            checker.REQUIRED_SYMBOLS["node"],
        )
        == 23
    )

    source = complete.read_text(encoding="utf-8")
    for symbol in (
        "inspectSorafsOrderbookSubmissionForDiscriminantV1",
        "verifySorafsOrderbookSubmissionReceiptV1",
    ):
        incomplete = tmp_path / f"missing-{symbol}.cjs"
        incomplete.write_text(
            re.sub(rf"{symbol}\(\) \{{\}},?", "", source), encoding="utf-8"
        )
        with pytest.raises(checker.ArtifactContractError, match="missing required exports"):
            checker.probe_node_abi(incomplete, checker.REQUIRED_SYMBOLS["node"])

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
        "    return 23\n"
        "def inspect_sorafs_orderbook_submission_for_discriminant_v1():\n"
        "    return {}\n"
        "def sorafs_validate_appeal_finance_cancel_asset_lock_json():\n"
        "    return '{}'\n"
        "def verify_sorafs_orderbook_submission_receipt_v1():\n"
        "    return '{}'\n",
        encoding="utf-8",
    )
    assert (
        checker.probe_python_abi(
            complete,
            checker.REQUIRED_SYMBOLS["python"],
        )
        == 23
    )

    source = complete.read_text(encoding="utf-8")
    for symbol in (
        "inspect_sorafs_orderbook_submission_for_discriminant_v1",
        "verify_sorafs_orderbook_submission_receipt_v1",
    ):
        incomplete = tmp_path / f"missing-{symbol}.py"
        incomplete.write_text(
            re.sub(rf"def {symbol}\(\):\n    return .*\n", "", source),
            encoding="utf-8",
        )
        with pytest.raises(checker.ArtifactContractError, match="missing required exports"):
            checker.probe_python_abi(incomplete, checker.REQUIRED_SYMBOLS["python"])

    complete.write_text(
        "def connect_norito_bridge_abi_version():\n"
        "    return '23'\n"
        "def inspect_sorafs_orderbook_submission_for_discriminant_v1():\n"
        "    return {}\n"
        "def sorafs_validate_appeal_finance_cancel_asset_lock_json():\n"
        "    return '{}'\n"
        "def verify_sorafs_orderbook_submission_receipt_v1():\n"
        "    return '{}'\n",
        encoding="utf-8",
    )
    with pytest.raises(checker.ArtifactContractError, match="ABI probe returned"):
        checker.probe_python_abi(complete, checker.REQUIRED_SYMBOLS["python"])


def run_gradle_jvm_hermetic_probe(
    tmp_path: Path,
    *,
    require_sorafs_native_validation: bool,
    profile: str = "gradle-jvm",
    include_localnet_dir: bool = False,
    include_localnet_test: bool = False,
) -> subprocess.CompletedProcess[str]:
    """Run a dependency-free probe through the exact Gradle/JVM environment."""

    python = Path(sys.executable).resolve(strict=True)
    native = tmp_path / "native"
    environment = {
        "ANDROID_HOME": str(tmp_path / "android-sdk"),
        "ANDROID_SDK_ROOT": str(tmp_path / "android-sdk"),
        "DYLD_LIBRARY_PATH": str(native),
        "GRADLE_USER_HOME": str(tmp_path / "gradle-home"),
        "HOME": str(tmp_path / "home"),
        "IROHA_NATIVE_LIBRARY_PATH": str(native),
        "IROHA_REQUIRE_KAGEMUSHA_NATIVE": "1",
        "JAVA_HOME": str(tmp_path / "jdk"),
        "LANG": "C.UTF-8",
        "LC_ALL": "C.UTF-8",
        "LD_LIBRARY_PATH": str(native),
        "PATH": "/usr/bin:/bin",
        "TMPDIR": str(tmp_path),
    }
    if require_sorafs_native_validation:
        environment["IROHA_REQUIRE_SORAFS_NATIVE_VALIDATION"] = "1"
    if include_localnet_dir:
        environment["IROHA_LOCALNET_DIR"] = str(tmp_path / "four-peer-localnet")
    if include_localnet_test:
        environment["IROHA_LOCALNET_TEST"] = "1"

    command = [
        str(python),
        "-I",
        "-S",
        str(REPO_ROOT / "scripts/run_mobile_hermetic_command.py"),
        "--profile",
        profile,
    ]
    for name, value in sorted(environment.items()):
        command.extend(("--set", f"{name}={value}"))
    command.extend(
        (
            "--",
            str(python),
            "-I",
            "-S",
            "-c",
            (
                "import json,os; names=("
                "'IROHA_REQUIRE_SORAFS_NATIVE_VALIDATION',"
                "'IROHA_LOCALNET_DIR','IROHA_LOCALNET_TEST'); "
                "print(json.dumps({name:os.environ.get(name) for name in names},"
                "sort_keys=True))"
            ),
        )
    )
    return subprocess.run(command, check=False, capture_output=True, text=True)


def test_gradle_jvm_hermetic_profile_forwards_sorafs_native_requirement(
    tmp_path: Path,
) -> None:
    """The release subprocess receives the mandatory fail-closed SoraFS flag."""

    completed = run_gradle_jvm_hermetic_probe(
        tmp_path,
        require_sorafs_native_validation=True,
    )
    assert completed.returncode == 0, completed.stderr
    forwarded = json.loads(completed.stdout)
    assert forwarded == {
        "IROHA_LOCALNET_DIR": None,
        "IROHA_LOCALNET_TEST": None,
        "IROHA_REQUIRE_SORAFS_NATIVE_VALIDATION": "1",
    }


def test_gradle_jvm_hermetic_profile_requires_sorafs_native_requirement(
    tmp_path: Path,
) -> None:
    """Omitting the SoraFS native requirement fails before the child executes."""

    completed = run_gradle_jvm_hermetic_probe(
        tmp_path,
        require_sorafs_native_validation=False,
    )
    assert completed.returncode == 1
    assert completed.stdout == ""
    assert "environment inventory is not exact" in completed.stderr
    assert "missing=['IROHA_REQUIRE_SORAFS_NATIVE_VALIDATION']" in completed.stderr


def test_gradle_jvm_localnet_profile_forwards_exact_runtime_handles(
    tmp_path: Path,
) -> None:
    """The localnet profile forwards reviewed handles without weakening JNI gates."""

    completed = run_gradle_jvm_hermetic_probe(
        tmp_path,
        require_sorafs_native_validation=True,
        profile="gradle-jvm-localnet",
        include_localnet_dir=True,
        include_localnet_test=True,
    )
    assert completed.returncode == 0, completed.stderr
    forwarded = json.loads(completed.stdout)
    assert forwarded == {
        "IROHA_LOCALNET_DIR": str(tmp_path / "four-peer-localnet"),
        "IROHA_LOCALNET_TEST": "1",
        "IROHA_REQUIRE_SORAFS_NATIVE_VALIDATION": "1",
    }


@pytest.mark.parametrize(
    "missing",
    ("IROHA_LOCALNET_DIR", "IROHA_LOCALNET_TEST"),
)
def test_gradle_jvm_localnet_profile_requires_both_runtime_handles(
    tmp_path: Path,
    missing: str,
) -> None:
    """Either missing localnet handle rejects the command before child execution."""

    completed = run_gradle_jvm_hermetic_probe(
        tmp_path,
        require_sorafs_native_validation=True,
        profile="gradle-jvm-localnet",
        include_localnet_dir=missing != "IROHA_LOCALNET_DIR",
        include_localnet_test=missing != "IROHA_LOCALNET_TEST",
    )
    assert completed.returncode == 1
    assert completed.stdout == ""
    assert "environment inventory is not exact" in completed.stderr
    assert f"missing=['{missing}']" in completed.stderr


def test_kotlin_localnet_release_lane_is_mandatory_and_payload_free() -> None:
    """Freeze four-peer execution, zero skips, teardown, and safe CI evidence."""

    runner = (REPO_ROOT / "scripts/run_mobile_hermetic_command.py").read_text(
        encoding="utf-8"
    )
    gate = (REPO_ROOT / "ci/check_kagemusha_jvm_native_bridge.sh").read_text(
        encoding="utf-8"
    )
    mobile = (REPO_ROOT / ".github/workflows/mobile_sdk_artifacts.yml").read_text(
        encoding="utf-8"
    )

    assert '"gradle-jvm-localnet": GRADLE_JVM_ENVIRONMENT' in runner
    assert '"IROHA_LOCALNET_DIR"' in runner
    assert '"IROHA_LOCALNET_TEST"' in runner
    for token in (
        'LOCALNET_DEPLOYER="$ROOT_DIR/scripts/deploy_localnet.sh"',
        'LOCALNET_TEST_CLASS="org.hyperledger.iroha.sdk.client.ZkAssetShieldLocalnetTest"',
        '--profile gradle-jvm-localnet',
        '--set "IROHA_LOCALNET_DIR=$LOCALNET_DIR"',
        '--set "IROHA_LOCALNET_TEST=1"',
        '"$CARGO_BINARY" build --locked --offline --target "$HOST_TRIPLE"',
        "-p iroha_kagami -p irohad -p iroha_cli",
        "--peers 4",
        "verify_four_peer_localnet",
        '"http://${torii_address}/health"',
        "stop_localnet || fail",
        'aggregate["skipped"] != 0',
        '"tests": 1, "skipped": 0, "failures": 0, "errors": 0',
        '"peer_count": 4',
        '"teardown_complete": True',
    ):
        assert token in gate
    assert gate.count('write_exclusive("') == 3
    assert 'write_exclusive("zk-asset-shield-localnet.junit.xml"' in gate
    assert 'write_exclusive("c-jni-native-abi22.json"' in gate
    assert 'write_exclusive("zk-asset-shield-localnet-summary.json"' in gate
    for forbidden in ("client.toml", "genesis.json", "private_key"):
        assert f'write_exclusive("{forbidden}' not in gate

    assert "timeout-minutes: 180" in mobile
    assert 'KAGEMUSHA_JVM_NATIVE_EVIDENCE_DIR: ${{ runner.temp }}/' in mobile
    assert "name: Upload Kotlin four-peer localnet evidence" in mobile
    assert "if: always()" in mobile
    assert "if-no-files-found: error" in mobile
    assert '"scripts/deploy_localnet.sh"' in mobile

    kagemusha_paths = pull_request_paths("pr_kagemusha_payload_bench.yml")
    assert {
        "Cargo.toml",
        "crates/iroha_cli/**",
        "crates/iroha_config/**",
        "crates/iroha_core/**",
        "crates/irohad/**",
        "kotlin/core-jvm/**",
        "scripts/deploy_localnet.sh",
        "scripts/*taira*.py",
        "scripts/operator_http_headers.py",
        "scripts/release_artifact_contract.py",
        "scripts/tests/*taira*_test.py",
        "scripts/run_mobile_hermetic_command.py",
    } <= kagemusha_paths


def kotlin_localnet_evidence_program() -> str:
    """Extract the dependency-free JUnit/evidence checker embedded in the gate."""

    gate = (REPO_ROOT / "ci/check_kagemusha_jvm_native_bridge.sh").read_text(
        encoding="utf-8"
    )
    start = '  "$HOST_TRIPLE" <<\'PY\'\n'
    end = "\nPY\n\nrun_full_suite java"
    assert gate.count(start) == 1
    program, separator, _ = gate.split(start, 1)[1].partition(end)
    assert separator == end
    return program


def write_junit(
    path: Path,
    *,
    suite: str,
    tests: int,
    skipped: int,
    skipped_node: bool | None = None,
) -> None:
    """Write one bounded Gradle-shaped JUnit fixture."""

    include_skipped_node = bool(skipped) if skipped_node is None else skipped_node
    skipped_xml = "<skipped/>" if include_skipped_node else ""
    path.write_text(
        f'<testsuite name="{suite}" tests="{tests}" skipped="{skipped}" '
        'failures="0" errors="0">'
        f'<testcase name="case()" classname="{suite}">{skipped_xml}</testcase>'
        "</testsuite>\n",
        encoding="utf-8",
    )


def run_kotlin_localnet_evidence_program(
    tmp_path: Path,
    *,
    target_skipped: int = 0,
    aggregate_skipped: int = 0,
    aggregate_skipped_node: bool | None = None,
) -> tuple[subprocess.CompletedProcess[str], Path]:
    """Run the embedded validator against synthetic payload-free reports."""

    expected_class = "org.hyperledger.iroha.sdk.client.ZkAssetShieldLocalnetTest"
    result_dir = tmp_path / "results"
    evidence_dir = tmp_path / "evidence"
    result_dir.mkdir()
    evidence_dir.mkdir()
    target = result_dir / f"TEST-{expected_class}.xml"
    write_junit(
        target,
        suite=expected_class,
        tests=1,
        skipped=target_skipped,
    )
    write_junit(
        result_dir / "TEST-release-companion.xml",
        suite="release-companion",
        tests=1,
        skipped=aggregate_skipped,
        skipped_node=aggregate_skipped_node,
    )
    host_target = "x86_64-unknown-linux-gnu"
    native = tmp_path / "c-jni-native-abi22.json"
    native.write_text(
        json.dumps(
            {
                "artifact_sha256": "a" * 64,
                "bridge_abi_version": 23,
                "sdk": "c-jni",
                "source_commit": "b" * 40,
                "source_tree_clean": True,
                "target": host_target,
            },
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    python = Path(sys.executable).resolve(strict=True)
    completed = subprocess.run(
        (
            str(python),
            "-I",
            "-S",
            "-",
            str(result_dir),
            str(target),
            str(native),
            str(evidence_dir),
            host_target,
        ),
        input=kotlin_localnet_evidence_program(),
        check=False,
        capture_output=True,
        text=True,
    )
    return completed, evidence_dir


def test_kotlin_localnet_evidence_validator_emits_only_safe_success_files(
    tmp_path: Path,
) -> None:
    """A clean no-skip result emits only JUnit, ABI, and summary evidence."""

    completed, evidence_dir = run_kotlin_localnet_evidence_program(tmp_path)
    assert completed.returncode == 0, completed.stderr
    assert {path.name for path in evidence_dir.iterdir()} == {
        "c-jni-native-abi22.json",
        "zk-asset-shield-localnet-summary.json",
        "zk-asset-shield-localnet.junit.xml",
    }
    summary = json.loads(
        (evidence_dir / "zk-asset-shield-localnet-summary.json").read_text(
            encoding="utf-8"
        )
    )
    assert summary["status"] == "passed"
    assert summary["peer_count"] == 4
    assert summary["teardown_complete"] is True
    assert summary["target_suite"] == {
        "errors": 0,
        "failures": 0,
        "name": "org.hyperledger.iroha.sdk.client.ZkAssetShieldLocalnetTest",
        "skipped": 0,
        "tests": 1,
    }
    assert summary["aggregate"]["skipped"] == 0


@pytest.mark.parametrize(
    ("target_skipped", "aggregate_skipped", "aggregate_skipped_node", "message"),
    (
        (1, 0, None, "JUnit counters are not release-ready"),
        (0, 1, None, "release suite may not contain skipped tests"),
        (0, 0, True, "counters do not match outcome nodes"),
    ),
)
def test_kotlin_localnet_evidence_validator_rejects_every_skip(
    tmp_path: Path,
    target_skipped: int,
    aggregate_skipped: int,
    aggregate_skipped_node: bool | None,
    message: str,
) -> None:
    """Neither the target integration case nor a companion may be skipped."""

    completed, evidence_dir = run_kotlin_localnet_evidence_program(
        tmp_path,
        target_skipped=target_skipped,
        aggregate_skipped=aggregate_skipped,
        aggregate_skipped_node=aggregate_skipped_node,
    )
    assert completed.returncode != 0
    assert message in completed.stderr
    assert list(evidence_dir.iterdir()) == []


def test_repository_wires_exact_abi23_release_contract() -> None:
    """Freeze the fail-closed source and CI wiring without loading a native binary."""

    def read(relative: str) -> str:
        return (REPO_ROOT / relative).read_text(encoding="utf-8")

    node_copy = read("javascript/iroha_js/scripts/copy-native.mjs")
    for token in (
        'export const REQUIRED_NATIVE_BRIDGE_ABI_VERSION = 23;',
        '"connectNoritoBridgeAbiVersion"',
        '"inspectSorafsOrderbookSubmissionForDiscriminantV1"',
        '"sorafsValidateAppealFinanceCancelAssetLockJson"',
        '"verifySorafsOrderbookSubmissionReceiptV1"',
        "buildProvenance.source_tree_clean !== true",
    ):
        assert token in node_copy

    python_native = read("python/iroha_python/iroha_python_rs/src/lib.rs")
    python_orderbook_native = read(
        "python/iroha_python/iroha_python_rs/src/sorafs_orderbook_submission.rs"
    )
    assert '#[pyo3(name = "connect_norito_bridge_abi_version")]' in python_native
    assert "fn connect_norito_bridge_abi_version_py() -> u32" in python_native
    assert "connect_norito_bridge_abi_version_py," in python_native
    assert (
        'name = "inspect_sorafs_orderbook_submission_for_discriminant_v1"'
        in python_orderbook_native
    )
    assert 'name = "verify_sorafs_orderbook_submission_receipt_v1"' in python_orderbook_native

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
    assert "REQUIRED_NATIVE_SIGNER_CONTRACT_REVISION: Int = 5" in kotlin_signer
    assert "REQUIRED_NATIVE_SIGNER_CONTRACT_REVISION = 5" in java_signer
    for relative in (
        "roadmap.md",
        "specs/sorafs/v1_closure_ledger.md",
        "specs/sorafs_reference_sdk_plan.md",
    ):
        normalized = " ".join(read(relative).replace("-", " ").split())
        assert "`NativeSignerBridge` JNI contract revision 5" in normalized
        for retired_revision in range(1, 5):
            assert (
                f"`NativeSignerBridge` JNI contract revision {retired_revision}"
                not in normalized
            )
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
        "ABI-23 connect_norito_bridge with all SoraFS reference symbols is required."
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
        r"native encoder|ABI-23 bridge|bridge functions)",
    )
    assert native_swift_skip.search(all_swift_tests) is None
    assert "func requireNativeTestCapability(" in all_swift_tests
    assert "RequiredNativeTestCapabilityError.unavailable" in all_swift_tests

    required_jvm_native_assertion = (
        "A freshly built connect_norito_bridge ABI 23 "
        "artifact-streaming library is required"
    )

    all_kotlin_tests = read_test_tree("kotlin/core-jvm/src/test", ".kt")
    assert "assumeTrue(" not in all_kotlin_tests
    assert "IROHA_REQUIRE_KAGEMUSHA_NATIVE" not in all_kotlin_tests
    assert "assertNativeArtifactStreamingUnavailableFailsClosed" not in all_kotlin_tests
    assert re.search(
        r"if\s*\(\s*!NativeSignerBridge\.isNativeAvailable\(\)\s*\)\s*return\b",
        all_kotlin_tests,
    ) is None
    assert required_jvm_native_assertion in all_kotlin_tests

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
    assert required_jvm_native_assertion in all_java_tests

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
        assert "check_native_sdk_abi22_artifact.py" in lane
        assert "record" in lane
        assert "verify" in lane
    assert 'PYTHON_VERSION}" != "3.12"' in python_lane
    assert "sys.version_info.major}{sys.version_info.minor}" in python_lane
    node_lane = read("ci/sdk_sorafs_orchestrator.sh")
    assert "native/iroha_js_host.node" in node_lane
    assert "--sdk node" in node_lane
    assert node_lane.count("check_native_sdk_abi22_artifact.py") == 2
    assert "record" in node_lane
    assert "verify" in node_lane

    mobile_checker = read("scripts/check_mobile_sdk_artifacts.sh")
    mobile_workflow = read(".github/workflows/mobile_sdk_artifacts.yml")
    assert '"native_bridge_abi_version"] != 23' in mobile_checker
    assert "check_mobile_sdk_artifacts.sh --apple-only" in mobile_workflow
    assert "check_kagemusha_jvm_native_bridge.sh" in mobile_workflow
    jni_lane = read("ci/check_kagemusha_jvm_native_bridge.sh")
    assert (
        f'REQUIRED_NATIVE_ASSERTION="{required_jvm_native_assertion}"' in jni_lane
    )
    assert 'ABI22_ARTIFACT_CHECKER="$ROOT_DIR/scripts/check_native_sdk_abi22_artifact.py"' in jni_lane
    assert "resolve_trusted_python312()" in jni_lane
    assert "MOBILE_SDK_PYTHON_BINARY" in jni_lane
    assert "sys.version_info[:2] != (3, 12)" in jni_lane
    assert 'if [[ -n "${NORITO_MOBILE_JAVA_HOME:-}" ]]; then' in jni_lane
    assert 'JAVA_HOME_DIR="$NORITO_MOBILE_JAVA_HOME"' in jni_lane
    assert (
        jni_lane.index('if [[ -n "${NORITO_MOBILE_JAVA_HOME:-}" ]]; then')
        < jni_lane.index("/usr/libexec/java_home -v 21")
    )
    assert (
        "NORITO_MOBILE_JAVA_HOME or the macOS Java locator must provide an "
        "absolute regular JDK directory"
    ) in jni_lane
    java_home_resolution = (
        'JAVA_HOME_DIR="$("$PYTHON_BINARY" -I -S -c '
        "'import pathlib,sys; "
        "print(pathlib.Path(sys.argv[1]).resolve(strict=True))' "
        '"$JAVA_HOME_DIR")"'
    )
    assert '[[ "$JAVA_HOME_DIR" == /* && -d "$JAVA_HOME_DIR" ]]' in jni_lane
    assert java_home_resolution in jni_lane
    assert (
        '[[ "$JAVA_HOME_DIR" == /* && -d "$JAVA_HOME_DIR" '
        '&& ! -L "$JAVA_HOME_DIR" ]]'
        in jni_lane
    )
    assert (
        "NORITO_MOBILE_JAVA_HOME or the macOS Java locator must resolve to a "
        "canonical regular JDK directory"
    ) in jni_lane
    for rust_tool in ("cargo", "rustc", "rustdoc"):
        assert (
            '"$RUSTUP_BINARY" which --toolchain "$PINNED_TOOLCHAIN" '
            f"{rust_tool}"
        ) in jni_lane
    assert 'NORITO_BRIDGE_SEAL_CARGO_TARGET_DIR="$CARGO_TARGET_DIR"' in jni_lane
    assert 'NORITO_BRIDGE_SEAL_RUSTDOC="$RUSTDOC_BINARY"' in jni_lane
    assert '"$rustdoc_commit" == "$rustc_commit"' in jni_lane
    assert '"$RUSTDOC_BINARY:$RUSTDOC_SHA256_START"' in jni_lane
    assert '"$PYTHON_BINARY" -I -S' in jni_lane
    assert '--set "IROHA_REQUIRE_SORAFS_NATIVE_VALIDATION=1"' in jni_lane
    assert "--sdk c-jni" in jni_lane
    for marker in (
        'BUILT_NATIVE_LIBRARY="$CARGO_TARGET_DIR/$HOST_TRIPLE/debug/',
        'NATIVE_LIBRARY_DIR="$BUILD_SESSION/c-jni-native"',
        'NATIVE_LIBRARY="$NATIVE_LIBRARY_DIR/${BUILT_NATIVE_LIBRARY##*/}"',
        '/bin/cp "$BUILT_NATIVE_LIBRARY" "$NATIVE_LIBRARY"',
        'chmod 0700 "$NATIVE_LIBRARY"',
    ):
        assert marker in jni_lane
    assert jni_lane.index('/bin/cp "$BUILT_NATIVE_LIBRARY" "$NATIVE_LIBRARY"') < (
        jni_lane.index('"$PYTHON_BINARY" -I -S "$ABI22_ARTIFACT_CHECKER" record')
    )
