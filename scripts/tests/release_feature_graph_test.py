"""Regression coverage for shipping Cargo feature isolation."""

from __future__ import annotations

import importlib.util
import os
import re
import subprocess
from pathlib import Path
from types import SimpleNamespace

import pytest

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - exercised on Python <3.11
    import tomli as tomllib


REPO = Path(__file__).resolve().parents[2]
SCRIPT = REPO / "scripts" / "check_release_feature_graph.py"
WORKFLOW = REPO / ".github" / "workflows" / "pr.yml"


def load_checker():
    spec = importlib.util.spec_from_file_location("release_feature_graph", SCRIPT)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def prepare_docker_parser_repo(tmp_path: Path, dockerfile: str | None = None) -> Path:
    workflow_dir = tmp_path / ".github" / "workflows"
    workflow_dir.mkdir(parents=True)
    for name in ("publish_custom.yml", "publish_dev.yml", "publish_xx.yml"):
        (workflow_dir / name).write_text("", encoding="utf-8")
    (workflow_dir / "ci_image.yml").write_text(
        """jobs:
  ci:
    steps:
      - uses: docker/build-push-action@pinned
        with:
          push: true
""",
        encoding="utf-8",
    )
    (tmp_path / "Dockerfile").write_text(
        dockerfile
        or """ARG FEATURES=""
ARG CARGOFLAGS=""
ARG BINARIES="demo"
RUN cargo ${CARGOFLAGS} build --features "${FEATURES}" --bin demo
""",
        encoding="utf-8",
    )
    return workflow_dir


def initialize_tracked_release_surface(destination: Path) -> None:
    sources = {
        ".cargo/config.toml": "[alias]\nxtask = 'run -p xtask --'\n",
        ".dockerignore": "target\n",
        "Dockerfile": "FROM scratch\n",
        ".github/actions/package/action.yml": "runs:\n  using: composite\n  steps: []\n",
        ".github/workflows/publish.yml": "on: push\n",
        "CHANGELOG.md": "# Changelog\n",
        "Cargo.lock": "# reviewed lock\n",
        "Cargo.toml": '[workspace]\nmembers = ["crates/demo"]\n',
        "LICENSE": "reviewed license\n",
        "cliff.toml": '[changelog]\nbody = "reviewed"\n',
        "flake.lock": '{"nodes":{},"root":"root","version":7}\n',
        "flake.nix": "{ outputs = _: {}; }\n",
        "IrohaSwift/IrohaSwift.podspec": "Pod::Spec.new do |spec|\nend\n",
        "IrohaSwift/Package.swift": "// swift-tools-version: 6.0\n",
        "IrohaSwift/Tests/IrohaSwiftTests/ArtifactTests.swift": "// reviewed\n",
        "crates/connect_norito_bridge/NoritoBridge.podspec.template": "Pod::Spec.new do |spec|\nend\n",
        "crates/demo/Cargo.toml": '[package]\nname = "demo"\nversion = "0.1.0"\n',
        "crates/demo/build.rs": 'fn main() { println!("cargo:rerun-if-changed=build_input.txt"); }\n',
        "crates/demo/build_input.txt": "reviewed build input\n",
        "crates/sorafs_manifest/include/sorafs_reference.h": "/* reviewed FFI header */\n",
        "codec/rans/tables/default.bin": "reviewed table\n",
        "configs/soranexus/taira/config.toml": "[network]\n",
        "configs/sorafs/external_software_signer/config.toml": "[signer]\n",
        "configs/sorafs/runtime_provider_broker/config.toml": "[broker]\n",
        "csharp/Directory.Build.props": "<Project />\n",
        "csharp/Directory.Packages.props": "<Project />\n",
        "csharp/global.json": "{}\n",
        "csharp/src/Hyperledger.Iroha.Sdk/Hyperledger.Iroha.Sdk.csproj": "<Project />\n",
        "csharp/tests/Hyperledger.Iroha.Sdk.Tests/ArtifactTests.cs": "// reviewed\n",
        "dashboards/alerts/fastpq_acceleration_rules.yml": "groups: []\n",
        "dashboards/alerts/tests/fastpq_acceleration_rules.test.yml": "rule_files: []\n",
        "defaults/genesis.template.json": "{}\n",
        "gradle/mobile-sdk-external-android-build.settings.gradle.kts": "// reviewed\n",
        "java/iroha_android/build.gradle.kts": "plugins {}\n",
        "java/iroha_android/gradle/wrapper/gradle-wrapper.properties": "distributionUrl=https://example.invalid/gradle.zip\n",
        "java/iroha_android/gradlew": "#!/bin/sh\nexec java \"$@\"\n",
        "java/iroha_android/settings.gradle.kts": "rootProject.name = \"mirror\"\n",
        "kotlin/buildSrc/src/main/kotlin/ReleaseOwner.kt": "// reviewed\n",
        "kotlin/gradle/libs.versions.toml": "[versions]\n",
        "kotlin/gradle/wrapper/gradle-wrapper.properties": "distributionUrl=https://example.invalid/gradle.zip\n",
        "kotlin/gradlew": "#!/bin/sh\nexec java \"$@\"\n",
        "kotlin/settings.gradle.kts": "rootProject.name = \"test\"\n",
        "nix-appimage/LICENCE": "reviewed licence\n",
        "nix-appimage/README.md": "# Reviewed AppImage helper\n",
        "nix-appimage/apprun.c": "int main(void) { return 0; }\n",
        "nix-appimage/bundle": "#!/bin/sh\nexit 0\n",
        "nix-appimage/default.nix": "(builtins.getFlake (toString ./.)).bundlers\n",
        "nix-appimage/flake.lock": '{"nodes":{},"root":"root","version":7}\n',
        "nix-appimage/flake.nix": "{ outputs = _: {}; }\n",
        "release/version-map.toml": '[versions]\nsorafs_cli = "0.1.0"\n',
        "rust-toolchain.toml": '[toolchain]\nchannel = "stable"\n',
        "scripts/package_release.sh": "#!/usr/bin/env bash\nset -euo pipefail\n",
        "specs/sorafs/runbooks/release_rollback_yank.md": "# Release rollback and yank\n",
        "ci/check_release.sh": "#!/usr/bin/env bash\nset -euo pipefail\n",
    }
    for relative, contents in sources.items():
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(contents, encoding="utf-8")
    (destination / "IrohaSwift" / "NoritoBridge.xcframework").symlink_to(
        "../dist/NoritoBridge.xcframework"
    )
    subprocess.run(["git", "init", "-q"], cwd=destination, check=True)
    subprocess.run(["git", "add", "."], cwd=destination, check=True)


def assert_seal_rejects(checker, repo: Path, baseline: str) -> None:
    try:
        checker.validate_trusted_release_surface(repo, baseline)
    except RuntimeError as error:
        assert "trusted release source surface drifted" in str(error)
    else:  # pragma: no cover - failure branch
        raise AssertionError("trusted release source drift was accepted")


def test_trusted_release_surface_matches_reviewed_seal() -> None:
    checker = load_checker()
    assert (
        checker.trusted_release_surface_digest(REPO)
        == checker.TRUSTED_RELEASE_SURFACE_SHA256
    )


def test_trusted_release_surface_rejects_duplicate_or_dynamic_seal_assignment() -> None:
    checker = load_checker()
    source = SCRIPT.read_bytes()
    duplicate = source + (
        b"\nTRUSTED_RELEASE_SURFACE_SHA256 = "
        b"hashlib.sha256(b'unreviewed').hexdigest()\n"
    )
    try:
        checker._release_surface_contents(SCRIPT.relative_to(REPO), duplicate)
    except RuntimeError as error:
        assert "exactly one top-level literal assignment" in str(error)
    else:  # pragma: no cover - failure branch
        raise AssertionError("duplicate dynamic release seal assignment was accepted")

    dynamic_only = re.sub(
        rb'TRUSTED_RELEASE_SURFACE_SHA256\s*=\s*\(\s*"[0-9a-f]{64}"\s*\)',
        b"TRUSTED_RELEASE_SURFACE_SHA256 = hashlib.sha256(b'unreviewed').hexdigest()",
        source,
        count=1,
    )
    try:
        checker._release_surface_contents(SCRIPT.relative_to(REPO), dynamic_only)
    except RuntimeError as error:
        assert "exactly one top-level literal assignment" in str(error)
    else:  # pragma: no cover - failure branch
        raise AssertionError("dynamic release seal assignment was accepted")

    digest_match = re.search(rb"[0-9a-f]{64}", source)
    assert digest_match is not None
    digest = digest_match.group(0)
    annotated = re.sub(
        rb'TRUSTED_RELEASE_SURFACE_SHA256\s*=\s*\(\s*"[0-9a-f]{64}"\s*\)',
        b'TRUSTED_RELEASE_SURFACE_SHA256: str = "' + digest + b'"',
        source,
        count=1,
    )
    annotated = (
        b'# TRUSTED_RELEASE_SURFACE_SHA256 = ("' + digest + b'")\n' + annotated
    )
    with pytest.raises(RuntimeError, match="exactly one top-level literal assignment"):
        checker._release_surface_contents(SCRIPT.relative_to(REPO), annotated)

    for rebinding in (
        b"\nfrom hashlib import sha256 as TRUSTED_RELEASE_SURFACE_SHA256\n",
        b"\nglobals()['TRUSTED_RELEASE_SURFACE_SHA256'] = "
        b"hashlib.sha256(b'unreviewed').hexdigest()\n",
    ):
        try:
            checker._release_surface_contents(
                SCRIPT.relative_to(REPO), source + rebinding
            )
        except RuntimeError as error:
            assert "exactly one top-level literal assignment" in str(error)
        else:  # pragma: no cover - failure branch
            raise AssertionError("alternate release seal rebinding was accepted")


def test_trusted_release_surface_commit_rejects_dirty_or_uncommitted_inputs(
    tmp_path: Path, monkeypatch
) -> None:
    checker = load_checker()
    commit = "a" * 40
    state = {"dirty": False, "tracked": b"tracked\0"}

    monkeypatch.setattr(checker, "validate_trusted_release_surface", lambda *_: None)
    monkeypatch.setattr(
        checker, "_embedded_release_surface_sha256", lambda: "b" * 64
    )
    monkeypatch.setattr(
        checker, "trusted_release_surface_paths", lambda _repo: (Path("tracked"),)
    )
    monkeypatch.setattr(
        checker, "trusted_release_surface_digest", lambda _repo: "b" * 64
    )

    def fake_run(command, **_kwargs):
        if command[1:3] == ["rev-parse", "HEAD"]:
            return SimpleNamespace(returncode=0, stdout=commit + "\n")
        if command[1:3] == ["diff", "--quiet"]:
            return SimpleNamespace(returncode=1 if state["dirty"] else 0)
        if command[1:3] == ["ls-files", "--cached"]:
            return SimpleNamespace(returncode=0, stdout=state["tracked"])
        raise AssertionError(command)

    monkeypatch.setattr(checker.subprocess, "run", fake_run)
    assert (
        checker.validate_trusted_release_surface_commit(tmp_path, commit)
        == "b" * 64
    )
    state["dirty"] = True
    with pytest.raises(RuntimeError, match="tracked working-tree drift"):
        checker.validate_trusted_release_surface_commit(tmp_path, commit)
    state["dirty"] = False
    state["tracked"] = b""
    with pytest.raises(RuntimeError, match="uncommitted or ignored inputs"):
        checker.validate_trusted_release_surface_commit(tmp_path, commit)


def test_trusted_release_surface_commit_rejects_late_digest_drift(
    tmp_path: Path, monkeypatch
) -> None:
    checker = load_checker()
    commit = "a" * 40
    digests = iter(("b" * 64, "c" * 64))
    monkeypatch.setattr(
        checker, "_embedded_release_surface_sha256", lambda: "b" * 64
    )
    monkeypatch.setattr(
        checker, "trusted_release_surface_digest", lambda _repo: next(digests)
    )
    monkeypatch.setattr(checker, "trusted_release_surface_paths", lambda _repo: ())

    def fake_run(command, **_kwargs):
        if command[1:3] == ["rev-parse", "HEAD"]:
            return SimpleNamespace(returncode=0, stdout=commit + "\n")
        if command[1:3] == ["diff", "--quiet"]:
            return SimpleNamespace(returncode=0)
        if command[1:3] == ["ls-files", "--cached"]:
            return SimpleNamespace(returncode=0, stdout=b"")
        raise AssertionError(command)

    monkeypatch.setattr(checker.subprocess, "run", fake_run)
    with pytest.raises(RuntimeError, match="changed during commit validation"):
        checker.validate_trusted_release_surface_commit(tmp_path, commit)


def test_trusted_release_surface_seal_rejects_drift_addition_and_removal(
    tmp_path: Path,
) -> None:
    checker = load_checker()
    initialize_tracked_release_surface(tmp_path)
    baseline = checker.trusted_release_surface_digest(tmp_path)

    dockerfile = tmp_path / "Dockerfile"
    original = dockerfile.read_bytes()
    evasions = (
        b"\ncopy dist/test-fixture-demo /usr/local/bin/iroha3d\n",
        b"\n copy dist/test-fixture-demo /usr/local/bin/iroha3d\n",
        b"\nENV EVIL=/usr/local/bin/iroha3d\nCOPY dist/test-fixture-demo $EVIL\n",
        b"\nARG EVIL=/usr/local/bin/iroha3d\nCOPY dist/test-fixture-demo ${EVIL}\n",
        b"\nCOPY dist/test-fixture-demo /usr/local/./bin/iroha3d\n",
        b"\nCOPY dist/test-fixture-demo /usr/local/lib/../bin/iroha3d\n",
        b"\nCOPY dist/test-fixture-demo //usr/local/bin/iroha3d\n",
        b"\nRUN dd if=dist/test-fixture-demo of=/usr/local/bin/iroha3d\n",
        b"\nRUN /bin/cp dist/test-fixture-demo /usr/local/bin/iroha3d\n",
        b"\nRUN tar -xf dist/test-fixture.tar -C /usr/local/bin\n",
    )
    for evasion in evasions:
        dockerfile.write_bytes(original + evasion)
        assert_seal_rejects(checker, tmp_path, baseline)
    dockerfile.write_bytes(original)

    package_script = tmp_path / "scripts" / "package_release.sh"
    script_original = package_script.read_bytes()
    package_script.write_bytes(
        script_original + b"cp dist/test-fixture-demo /release/iroha3d\n"
    )
    assert_seal_rejects(checker, tmp_path, baseline)
    package_script.write_bytes(script_original)

    added = tmp_path / ".github" / "workflows" / "new_publisher.yml"
    added.write_text("on: push\n", encoding="utf-8")
    subprocess.run(["git", "add", str(added)], cwd=tmp_path, check=True)
    assert_seal_rejects(checker, tmp_path, baseline)
    subprocess.run(
        ["git", "rm", "-q", "-f", "--cached", "--", str(added.relative_to(tmp_path))],
        cwd=tmp_path,
        check=True,
    )
    added.unlink()

    added_script = tmp_path / "scripts" / "unreviewed_packager.sh"
    added_script.write_text("#!/usr/bin/env bash\n", encoding="utf-8")
    assert_seal_rejects(checker, tmp_path, baseline)
    added_script.unlink()

    build_script = tmp_path / "crates" / "demo" / "build.rs"
    build_script_original = build_script.read_bytes()
    build_script.write_bytes(
        build_script_original
        + b'println!("cargo:rustc-cfg=feature=\\\"test-fixtures\\\"");\n'
    )
    assert_seal_rejects(checker, tmp_path, baseline)
    build_script.write_bytes(build_script_original)

    build_input = tmp_path / "crates" / "demo" / "build_input.txt"
    build_input_original = build_input.read_bytes()
    build_input.write_bytes(build_input_original + b"fixture-enabled\n")
    assert_seal_rejects(checker, tmp_path, baseline)
    build_input.write_bytes(build_input_original)

    taira_config = tmp_path / "configs" / "soranexus" / "taira" / "config.toml"
    taira_config_original = taira_config.read_bytes()
    taira_config.write_bytes(taira_config_original + b"fixture_mode = true\n")
    assert_seal_rejects(checker, tmp_path, baseline)
    taira_config.write_bytes(taira_config_original)

    default_genesis = tmp_path / "defaults" / "genesis.template.json"
    default_genesis_original = default_genesis.read_bytes()
    default_genesis.write_bytes(b'{"fixture":true}\n')
    assert_seal_rejects(checker, tmp_path, baseline)
    default_genesis.write_bytes(default_genesis_original)

    for relative in (
        *checker.SORAFS_CLI_VERBATIM_PAYLOADS,
        *checker.SORAFS_REFERENCE_VALIDATOR_VERBATIM_PAYLOADS,
    ):
        payload = tmp_path / relative
        payload_original = payload.read_bytes()
        payload.write_bytes(payload_original + b"unreviewed release payload drift\n")
        assert_seal_rejects(checker, tmp_path, baseline)
        payload.write_bytes(payload_original)

    nix_inputs = [
        *(tmp_path / relative for relative in checker.NIX_RELEASE_OWNER_PATHS),
        *sorted(
            path
            for path in (tmp_path / checker.NIX_APPIMAGE_OWNER_ROOT).rglob("*")
            if path.is_file()
        ),
    ]
    for nix_input in nix_inputs:
        nix_original = nix_input.read_bytes()
        nix_input.write_bytes(nix_original + b"# unreviewed Nix release drift\n")
        assert_seal_rejects(checker, tmp_path, baseline)
        nix_input.write_bytes(nix_original)

    for relative in checker.DOTNET_REPOSITORY_ANCESTOR_OWNER_PATHS:
        owner = tmp_path / relative
        owner.write_text(
            "<Project><Target Name=\"Injected\" /></Project>\n", encoding="utf-8"
        )
        assert_seal_rejects(checker, tmp_path, baseline)
        owner.unlink()
    for name in ("NuGet.Config", "nuget.config", "NUGET.CONFIG"):
        nuget_config = tmp_path / name
        nuget_config.write_text("<configuration />\n", encoding="utf-8")
        assert_seal_rejects(checker, tmp_path, baseline)
        nuget_config.unlink()

    for relative in checker.RELEASE_PIPELINE_SEMANTIC_INPUTS:
        semantic_input = tmp_path / relative
        semantic_input_original = semantic_input.read_bytes()
        semantic_input.write_bytes(semantic_input_original + b"unreviewed release behavior drift\n")
        assert_seal_rejects(checker, tmp_path, baseline)
        semantic_input.write_bytes(semantic_input_original)

    for relative in checker.OPTIONAL_SIGNED_RELEASE_EVIDENCE_INPUTS:
        evidence_input = tmp_path / relative
        evidence_input_original = evidence_input.read_bytes()
        evidence_input.write_bytes(evidence_input_original + b"unreviewed signed evidence drift\n")
        assert_seal_rejects(checker, tmp_path, baseline)
        evidence_input.write_bytes(evidence_input_original)

    cargo_config = tmp_path / ".cargo" / "config.toml"
    cargo_config_original = cargo_config.read_bytes()
    cargo_config.write_bytes(
        cargo_config_original
        + b'\n[build]\nrustc-wrapper = "../tools/feature-injector.sh"\n'
    )
    wrapper = tmp_path / "tools" / "feature-injector.sh"
    wrapper.parent.mkdir(parents=True)
    wrapper.write_text("#!/bin/sh\nexec \"$@\" --cfg 'feature=\"test-fixtures\"'\n")
    assert_seal_rejects(checker, tmp_path, baseline)
    cargo_config.write_bytes(cargo_config_original)
    wrapper.unlink()
    wrapper.parent.rmdir()

    gradle_wrapper = tmp_path / "kotlin" / "gradlew"
    gradle_wrapper_original = gradle_wrapper.read_bytes()
    gradle_wrapper.write_bytes(
        gradle_wrapper_original + b"cp /tmp/fixture.so artifacts/libnorito.so\n"
    )
    assert_seal_rejects(checker, tmp_path, baseline)
    gradle_wrapper.write_bytes(gradle_wrapper_original)

    build_src = (
        tmp_path / "kotlin" / "buildSrc" / "src" / "main" / "kotlin" / "ReleaseOwner.kt"
    )
    build_src_original = build_src.read_bytes()
    build_src.write_bytes(
        build_src_original + b"// inject fixture-enabled native task\n"
    )
    assert_seal_rejects(checker, tmp_path, baseline)
    build_src.write_bytes(build_src_original)

    applied_settings = (
        tmp_path / "gradle" / "mobile-sdk-external-android-build.settings.gradle.kts"
    )
    applied_settings_original = applied_settings.read_bytes()
    applied_settings.write_bytes(
        applied_settings_original + b"// substitute fixture artifact\n"
    )
    assert_seal_rejects(checker, tmp_path, baseline)
    applied_settings.write_bytes(applied_settings_original)

    java_wrapper = tmp_path / "java" / "iroha_android" / "gradlew"
    java_wrapper_original = java_wrapper.read_bytes()
    java_wrapper.write_bytes(
        java_wrapper_original
        + b"cp /tmp/fixture.aar \"$MOBILE_SDK_ANDROID_ARTIFACT_DIR/client.aar\"\n"
    )
    assert_seal_rejects(checker, tmp_path, baseline)
    java_wrapper.write_bytes(java_wrapper_original)

    java_build = tmp_path / "java" / "iroha_android" / "build.gradle.kts"
    java_build_original = java_build.read_bytes()
    java_build.write_bytes(
        java_build_original
        + b'tasks.named("test").configure { doLast { copy { from("/tmp/fixture.aar") } } }\n'
    )
    assert_seal_rejects(checker, tmp_path, baseline)
    java_build.write_bytes(java_build_original)

    swift_manifest = tmp_path / "IrohaSwift" / "Package.swift"
    swift_manifest_original = swift_manifest.read_bytes()
    swift_manifest.write_bytes(
        swift_manifest_original
        + b'let _ = Process.run(URL(fileURLWithPath: "/bin/cp"), arguments: ["/tmp/fixture", "artifact"])\n'
    )
    assert_seal_rejects(checker, tmp_path, baseline)
    swift_manifest.write_bytes(swift_manifest_original)

    swift_test = (
        tmp_path / "IrohaSwift" / "Tests" / "IrohaSwiftTests" / "ArtifactTests.swift"
    )
    swift_test_original = swift_test.read_bytes()
    swift_test.write_bytes(
        swift_test_original + b"// replace the external XCFramework during swift test\n"
    )
    assert_seal_rejects(checker, tmp_path, baseline)
    swift_test.write_bytes(swift_test_original)

    source_podspec = tmp_path / "IrohaSwift" / "IrohaSwift.podspec"
    source_podspec_original = source_podspec.read_bytes()
    source_podspec.write_bytes(
        source_podspec_original
        + b"spec.prepare_command = 'cp /tmp/fixture artifact'\n"
    )
    assert_seal_rejects(checker, tmp_path, baseline)
    source_podspec.write_bytes(source_podspec_original)

    binary_podspec_template = (
        tmp_path
        / "crates"
        / "connect_norito_bridge"
        / "NoritoBridge.podspec.template"
    )
    binary_podspec_template_original = binary_podspec_template.read_bytes()
    binary_podspec_template.write_bytes(
        binary_podspec_template_original
        + b"spec.prepare_command = 'cp /tmp/fixture artifact'\n"
    )
    assert_seal_rejects(checker, tmp_path, baseline)
    binary_podspec_template.write_bytes(binary_podspec_template_original)

    csharp_project = (
        tmp_path
        / "csharp"
        / "src"
        / "Hyperledger.Iroha.Sdk"
        / "Hyperledger.Iroha.Sdk.csproj"
    )
    csharp_project_original = csharp_project.read_bytes()
    csharp_project.write_bytes(
        csharp_project_original
        + b'<Target Name="ReplaceNative" AfterTargets="Test"><Copy SourceFiles="/tmp/fixture" DestinationFolder="artifacts" /></Target>\n'
    )
    assert_seal_rejects(checker, tmp_path, baseline)
    csharp_project.write_bytes(csharp_project_original)

    csharp_test = (
        tmp_path
        / "csharp"
        / "tests"
        / "Hyperledger.Iroha.Sdk.Tests"
        / "ArtifactTests.cs"
    )
    csharp_test_original = csharp_test.read_bytes()
    csharp_test.write_bytes(
        csharp_test_original + b"// replace staged native input during dotnet test\n"
    )
    assert_seal_rejects(checker, tmp_path, baseline)
    csharp_test.write_bytes(csharp_test_original)

    local_action = tmp_path / ".github" / "actions" / "package" / "action.yml"
    local_action_original = local_action.read_bytes()
    local_action.write_bytes(local_action_original + b"# replace package output\n")
    assert_seal_rejects(checker, tmp_path, baseline)
    local_action.write_bytes(local_action_original)

    development_bridge = tmp_path / "IrohaSwift" / "NoritoBridge.xcframework"
    development_bridge.unlink()
    development_bridge.symlink_to("../dist/fixture-enabled.xcframework")
    assert_seal_rejects(checker, tmp_path, baseline)
    development_bridge.unlink()
    development_bridge.symlink_to("../dist/NoritoBridge.xcframework")

    unexpected_symlink = (
        tmp_path / "IrohaSwift" / "Tests" / "IrohaSwiftTests" / "Injected.swift"
    )
    unexpected_symlink.symlink_to("ArtifactTests.swift")
    assert_seal_rejects(checker, tmp_path, baseline)
    unexpected_symlink.unlink()

    removed = tmp_path / "ci" / "check_release.sh"
    subprocess.run(
        [
            "git",
            "rm",
            "-q",
            "-f",
            "--cached",
            "--",
            str(removed.relative_to(tmp_path)),
        ],
        cwd=tmp_path,
        check=True,
    )
    removed.unlink()
    assert_seal_rejects(checker, tmp_path, baseline)


def test_trusted_release_surface_rejects_hardlinked_and_shared_writable_inputs(
    tmp_path: Path,
) -> None:
    checker = load_checker()
    initialize_tracked_release_surface(tmp_path)
    dockerfile = tmp_path / "Dockerfile"
    hardlink = tmp_path / "outside-dockerfile-link"
    os.link(dockerfile, hardlink)
    with pytest.raises(RuntimeError, match="exactly one hard link"):
        checker.trusted_release_surface_digest(tmp_path)
    hardlink.unlink()

    dockerfile.chmod(0o664)
    with pytest.raises(RuntimeError, match="group- or world-writable"):
        checker.trusted_release_surface_digest(tmp_path)
    dockerfile.chmod(0o644)

    source_directory = tmp_path / "crates" / "demo"
    source_directory.chmod(0o777)
    with pytest.raises(RuntimeError, match="source parent.*world-writable"):
        checker.trusted_release_surface_digest(tmp_path)
    source_directory.chmod(0o755)


def test_trusted_release_surface_covers_all_tracked_release_support() -> None:
    checker = load_checker()
    sealed = set(checker.trusted_release_surface_paths(REPO))
    tracked = subprocess.run(
        [
            "git",
            "ls-files",
            "-z",
            "--",
            ".cargo",
            ".github/actions",
            "IrohaSwift",
            "scripts",
            "ci",
            "codec/rans/tables",
            "configs/soranexus/taira",
            "configs/sorafs/external_software_signer",
            "configs/sorafs/runtime_provider_broker",
            "csharp",
            "defaults",
            "flake.lock",
            "flake.nix",
            "gradle",
            "java",
            "kotlin",
            "nix-appimage",
            ":(glob)**/Cargo.lock",
            ":(glob)**/Cargo.toml",
            ":(glob)**/build.rs",
            ":(glob)**/rust-toolchain*",
        ],
        cwd=REPO,
        check=True,
        capture_output=True,
    ).stdout
    support = {
        relative
        for raw in tracked.split(b"\0")
        if raw
        for relative in (Path(raw.decode("utf-8")),)
        if os.path.lexists(REPO / relative)
    }
    assert support <= sealed
    build_scripts = subprocess.run(
        ["git", "ls-files", "-z", "--", ":(glob)**/build.rs"],
        cwd=REPO,
        check=True,
        capture_output=True,
    ).stdout
    for raw in build_scripts.split(b"\0"):
        if not raw:
            continue
        build_script = Path(raw.decode("utf-8"))
        package_root = build_script.parent
        if not os.path.lexists(REPO / build_script):
            continue
        package_files = subprocess.run(
            ["git", "ls-files", "-z", "--", str(package_root)],
            cwd=REPO,
            check=True,
            capture_output=True,
        ).stdout
        assert {
            relative
            for item in package_files.split(b"\0")
            if item
            for relative in (Path(item.decode("utf-8")),)
            if os.path.lexists(REPO / relative)
        } <= sealed
    assert {
        Path(".cargo/config.toml"),
        Path("Cargo.lock"),
        Path("Cargo.toml"),
        Path("CHANGELOG.md"),
        Path("LICENSE"),
        Path("cliff.toml"),
        Path("dashboards/alerts/fastpq_acceleration_rules.yml"),
        Path("dashboards/alerts/tests/fastpq_acceleration_rules.test.yml"),
        Path("IrohaSwift/IrohaSwift.podspec"),
        Path("IrohaSwift/Package.swift"),
        Path("IrohaSwift/Package.resolved"),
        Path("crates/connect_norito_bridge/NoritoBridge.podspec.template"),
        Path("crates/iroha_core/build.rs"),
        Path("crates/iroha_core/src/state.rs"),
        Path("crates/sorafs_manifest/include/sorafs_reference.h"),
        Path("csharp/Directory.Build.props"),
        Path("csharp/Directory.Packages.props"),
        Path("csharp/global.json"),
        Path("defaults/genesis.template.json"),
        Path("csharp/src/Hyperledger.Iroha.Sdk/Hyperledger.Iroha.Sdk.csproj"),
        Path("flake.nix"),
        Path("scripts/package_sorafs_cli_candidate.py"),
        Path("scripts/package_sorafs_validate_release.sh"),
        Path("scripts/package_mobile_sdk_artifacts.sh"),
        Path("release/version-map.toml"),
        Path("specs/sorafs/runbooks/release_rollback_yank.md"),
        Path("ci/check_sorafs_cli_release.sh"),
        Path("gradle/mobile-sdk-external-android-build.settings.gradle.kts"),
        Path("java/iroha_android/build.gradle.kts"),
        Path("java/iroha_android/gradle/wrapper/gradle-wrapper.jar"),
        Path("java/iroha_android/gradle/wrapper/gradle-wrapper.properties"),
        Path("java/iroha_android/gradlew"),
        Path("java/iroha_android/gradlew.bat"),
        Path("java/iroha_android/settings.gradle.kts"),
        Path("kotlin/client-android/build.gradle.kts"),
        Path("kotlin/gradle/libs.versions.toml"),
        Path("kotlin/gradle/wrapper/gradle-wrapper.jar"),
        Path("kotlin/gradle/wrapper/gradle-wrapper.properties"),
        Path("kotlin/gradlew"),
        Path("kotlin/gradlew.bat"),
        Path("kotlin/settings.gradle.kts"),
        Path("nix-appimage/apprun.c"),
        Path("nix-appimage/bundle"),
        Path("nix-appimage/default.nix"),
        Path("nix-appimage/flake.lock"),
        Path("nix-appimage/flake.nix"),
        Path("rust-toolchain.toml"),
    } <= sealed


def test_trusted_release_surface_includes_ignored_autoloaded_controls(
    tmp_path: Path,
) -> None:
    checker = load_checker()
    initialize_tracked_release_surface(tmp_path)
    baseline = checker.trusted_release_surface_digest(tmp_path)
    (tmp_path / ".gitignore").write_text(
        ".cargo/config\nDirectory.Build.targets\n", encoding="utf-8"
    )
    cargo_override = tmp_path / ".cargo" / "config"
    cargo_override.write_text(
        '[build]\nrustc-wrapper = "../tools/fixture-injector"\n', encoding="utf-8"
    )
    msbuild_override = tmp_path / "Directory.Build.targets"
    msbuild_override.write_text(
        '<Project><Target Name="ReplaceReleaseOutput" /></Project>\n',
        encoding="utf-8",
    )

    inventoried = set(checker.trusted_release_surface_paths(tmp_path))
    assert Path(".cargo/config") in inventoried
    assert Path("Directory.Build.targets") in inventoried
    assert checker.trusted_release_surface_digest(tmp_path) != baseline

    cargo_override.unlink()
    msbuild_override.unlink()
    assert checker.trusted_release_surface_digest(tmp_path) == baseline


def _nix_test_catalog(checker):
    return checker.WorkspaceCatalog(
        package_features={
            "irohad": frozenset({"safe"}),
            "iroha_cli": frozenset(),
            "iroha_kagami": frozenset(),
            "iroha_data_model": frozenset({"test-fixtures"}),
        },
        binaries={
            "iroha3d": (checker.CargoBinary("irohad", "iroha3d", ()),),
            "iroha": (checker.CargoBinary("iroha_cli", "iroha", ()),),
            "kagami": (checker.CargoBinary("iroha_kagami", "kagami", ()),),
        },
        native_libraries={},
        workspace_docker_bins=(),
    )


def test_nix_named_outputs_are_bounded_shipping_profiles(tmp_path: Path) -> None:
    checker = load_checker()
    catalog = _nix_test_catalog(checker)
    targets = checker.nix_shipping_targets(REPO, catalog)
    iroha3_targets = {
        (target.package, target.binary, target.features)
        for target in targets
        if target.source.endswith(":packages.iroha3")
    }
    assert iroha3_targets == {
        ("irohad", "iroha3d", ()),
        ("iroha_cli", "iroha", ()),
        ("iroha_kagami", "kagami", ()),
    }
    assert {
        (target.package, target.binary, target.features)
        for target in targets
        if target.source.endswith(":packages.targets")
    } == {("irohad", "iroha3d", ())}

    source = (REPO / checker.NIX_RELEASE_OWNER).read_text(encoding="utf-8")
    helper = tmp_path / checker.NIX_APPIMAGE_OWNER_ROOT / "flake.nix"
    helper.parent.mkdir(parents=True)
    helper.write_text(
        (REPO / checker.NIX_APPIMAGE_OWNER_ROOT / "flake.nix").read_text(
            encoding="utf-8"
        ),
        encoding="utf-8",
    )
    safe = source.replace(
        "features = [];", 'features = ["irohad/safe"];', 1
    )
    assert safe != source
    (tmp_path / checker.NIX_RELEASE_OWNER).write_text(safe, encoding="utf-8")
    safe_targets = checker.nix_shipping_targets(tmp_path, catalog)
    assert any(
        target.source.endswith(":packages.iroha3")
        and target.package == "irohad"
        and target.features == ("safe",)
        for target in safe_targets
    )
    assert all(
        target.features == ()
        for target in safe_targets
        if target.source.endswith(":packages.iroha3") and target.package != "irohad"
    )

    unsafe_sources = (
        source.replace(
            "features = [];",
            'features = ["iroha_data_model/test-fixtures"];',
            1,
        ),
        source.rsplit("features = [];", 1)[0]
        + 'features = ["iroha_data_model/test-fixtures"];'
        + source.rsplit("features = [];", 1)[1],
    )
    for unsafe in unsafe_sources:
        (tmp_path / checker.NIX_RELEASE_OWNER).write_text(unsafe, encoding="utf-8")
        try:
            checker.nix_shipping_targets(tmp_path, catalog)
        except RuntimeError as error:
            assert "is not a selected shipping package" in str(error)
        else:  # pragma: no cover - failure branch
            raise AssertionError("foreign Nix shipping feature was accepted")

    unexpected_output = source.replace(
        "packages.default = packages.iroha3;",
        "packages.fixture = mkIroha { features = []; };\n\n"
        "      packages.default = packages.iroha3;",
        1,
    )
    assert unexpected_output != source
    (tmp_path / checker.NIX_RELEASE_OWNER).write_text(
        unexpected_output, encoding="utf-8"
    )
    try:
        checker.nix_shipping_targets(tmp_path, catalog)
    except RuntimeError as error:
        assert "named package outputs require explicit guard support" in str(error)
    else:  # pragma: no cover - failure branch
        raise AssertionError("unclassified Nix package output was accepted")

    alternate_drv = source.replace(
        "drv = packages.iroha3;", "drv = packages.targets;", 1
    )
    assert alternate_drv != source
    (tmp_path / checker.NIX_RELEASE_OWNER).write_text(
        alternate_drv, encoding="utf-8"
    )
    try:
        checker.nix_shipping_targets(tmp_path, catalog)
    except RuntimeError as error:
        assert "must wrap packages.iroha3 exactly" in str(error)
    else:  # pragma: no cover - failure branch
        raise AssertionError("alternate AppImage derivation was accepted")

    quoted_output = source.replace(
        "packages.default = packages.iroha3;",
        'packages."fixture" = mkIroha { features = []; };\n\n'
        "      packages.default = packages.iroha3;",
        1,
    )
    dynamic_output = source.replace(
        "packages.default = packages.iroha3;",
        '${"packages"}.fixture = mkIroha { features = []; };\n\n'
        "      packages.default = packages.iroha3;",
        1,
    )
    injected_options = source.replace(
        '++ ["--target" targetTriple]',
        '++ ["--features" "irohad/safe"]\n            ++ ["--target" targetTriple]',
        1,
    )
    spoofed_appimage = source.replace(
        "drv = packages.iroha3;",
        'drv = packages.iroha3;\n        "drv" = packages.targets;',
        1,
    )
    for unsafe in (
        quoted_output,
        dynamic_output,
        injected_options,
        spoofed_appimage,
    ):
        (tmp_path / checker.NIX_RELEASE_OWNER).write_text(unsafe, encoding="utf-8")
        try:
            checker.nix_shipping_targets(tmp_path, catalog)
        except RuntimeError:
            pass
        else:  # pragma: no cover - failure branch
            raise AssertionError("opaque Nix shipping surface was accepted")

    helper_source = helper.read_text(encoding="utf-8")
    helper.write_text(
        helper_source.replace(
            "closure = pkgs.writeReferencesToFile drv;",
            "closure = pkgs.writeReferencesToFile packages.runtime;",
            1,
        ),
        encoding="utf-8",
    )
    (tmp_path / checker.NIX_RELEASE_OWNER).write_text(source, encoding="utf-8")
    try:
        checker.nix_shipping_targets(tmp_path, catalog)
    except RuntimeError as error:
        assert "AppImage derivation provenance contract changed" in str(error)
    else:  # pragma: no cover - failure branch
        raise AssertionError("AppImage helper detached from drv was accepted")


def test_feature_graph_queries_all_targets(monkeypatch, tmp_path: Path) -> None:
    checker = load_checker()
    commands: list[list[str]] = []
    hostile = {
        "RUSTFLAGS": '--cfg feature="test-fixtures"',
        "CARGO_ENCODED_RUSTFLAGS": '--cfg\x1ffeature="test-fixtures"',
        "RUSTC_WRAPPER": "/tmp/unreviewed-wrapper",
        "CARGO_HOME": "/tmp/unreviewed-cargo-home",
        "CARGO_BUILD_RUSTC": "/tmp/unreviewed-rustc",
        "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER": "/tmp/linker",
    }
    for name, value in hostile.items():
        monkeypatch.setenv(name, value)
    monkeypatch.setenv("CARGO_TARGET_DIR", "/tmp/reviewed-target-dir")

    def fake_run(command, **kwargs):
        commands.append(command)
        environment = kwargs.pop("env")
        assert not set(hostile).intersection(environment)
        assert environment["CARGO_TARGET_DIR"] == "/tmp/reviewed-target-dir"
        assert kwargs == {
            "cwd": tmp_path,
            "check": False,
            "capture_output": True,
            "text": True,
        }
        return subprocess.CompletedProcess(command, 0, "reviewed graph\n", "")

    monkeypatch.setattr(checker.subprocess, "run", fake_run)
    assert (
        checker.feature_graph(tmp_path, "demo", ("safe",), False)
        == "reviewed graph\n"
    )
    assert commands == [
        [
            "cargo",
            "tree",
            "--locked",
            "--target",
            "all",
            "--package",
            "demo",
            "--edges",
            "normal,build,features",
            "--prefix",
            "none",
            "--no-default-features",
            "--features",
            "safe",
        ]
    ]


def test_shipping_packages_exclude_test_fixtures() -> None:
    checker = load_checker()
    for profile in checker.shipping_profiles(REPO):
        graph = checker.feature_graph(
            REPO,
            profile.package,
            profile.features,
            profile.default_features,
        )
        assert all(feature not in graph for feature in checker.FORBIDDEN_FEATURES)


def test_shipping_proof_consumers_keep_complete_parallel_engine() -> None:
    checker = load_checker()
    profiles = checker.shipping_profiles(REPO)
    for package, required_features in checker.REQUIRED_FEATURES.items():
        package_profiles = [profile for profile in profiles if profile.package == package]
        assert package_profiles
        for profile in package_profiles:
            graph = checker.feature_graph(
                REPO,
                profile.package,
                profile.features,
                profile.default_features,
            )
            assert all(feature in graph for feature in required_features)


def test_release_package_inventory_is_derived_from_shipping_declarations() -> None:
    checker = load_checker()
    targets = checker.declared_shipping_targets(REPO)
    profiles = set(checker.shipping_profiles(REPO))
    for target in targets:
        assert checker.ShippingProfile(
            package=target.package,
            features=target.features,
            default_features=target.default_features,
        ) in profiles

    profile_packages = {profile.package for profile in profiles}
    assert set(checker.BASELINE_PACKAGES) <= profile_packages
    assert {target.package for target in targets} <= profile_packages


def test_shipping_declarations_cover_docker_and_sorafs_release_surfaces() -> None:
    checker = load_checker()
    target_list = checker.declared_shipping_targets(REPO)

    def target_for(source_suffix: str, binary: str):
        matches = [
            target
            for target in target_list
            if target.source.endswith(source_suffix) and target.binary == binary
        ]
        assert matches
        return matches[0]

    attachment = target_for("->Dockerfile", "attachment_sanitizer")
    assert attachment.package == "iroha_torii"
    assert "app_api" in attachment.features

    docker_signer = target_for("->Dockerfile", "sorafs_external_software_signer")
    assert docker_signer.package == "irohad"
    assert "external-software-signer-bin" in docker_signer.features

    workflow = str(checker.SORAFS_RELEASE_WORKFLOW)
    assert target_for(workflow, "sorafs_cli").package == "sorafs_orchestrator"
    fetch = target_for(workflow, "sorafs_fetch")
    assert fetch.package == "sorafs_car"
    assert "cli" in fetch.features
    assert target_for(workflow, "sorafs-validate").package == "sorafs_manifest"
    workflow_signer = target_for(workflow, "sorafs_external_software_signer")
    assert workflow_signer.package == "irohad"
    assert "external-software-signer-bin" in workflow_signer.features


def test_published_docker_variants_and_feature_overrides_are_derived() -> None:
    checker = load_checker()
    invocations = checker.docker_publish_invocations(REPO)
    assert any(
        invocation.workflow == ".github/workflows/publish_dev.yml"
        for invocation in invocations
    )
    assert {invocation.dockerfile for invocation in invocations} == {
        "Dockerfile",
        "Dockerfile.musl",
        "Dockerfile.cross",
    }
    profiling = [
        invocation for invocation in invocations if invocation.features == ("profiling",)
    ]
    assert {invocation.dockerfile for invocation in profiling} == {
        "Dockerfile",
        "Dockerfile.cross",
    }
    profiles = set(checker.shipping_profiles(REPO))
    assert checker.ShippingProfile("iroha_core", ("profiling",)) in profiles
    assert checker.ShippingProfile("iroha_torii", ("profiling",)) in profiles
    assert checker.ShippingProfile(
        "iroha_torii", ("app_api", "profiling")
    ) in profiles


def test_published_docker_scope_override_is_rejected(tmp_path: Path) -> None:
    checker = load_checker()
    workflow_dir = prepare_docker_parser_repo(tmp_path)
    for cargo_flags in ("--workspace", "--features iroha/test-fixtures"):
        (workflow_dir / "publish.yml").write_text(
            f"""jobs:
  release:
    steps:
      - name: malicious scope expansion
        uses: docker/build-push-action@pinned
        with:
          context: .
          build-args: |
            "CARGOFLAGS={cargo_flags}"
""",
            encoding="utf-8",
        )
        try:
            checker.docker_publish_invocations(tmp_path)
        except RuntimeError as error:
            assert "may not expand" in str(error)
        else:  # pragma: no cover - failure branch
            raise AssertionError("Docker CARGOFLAGS scope expansion was accepted")


def test_dockerfile_hardcoded_all_features_is_rejected() -> None:
    checker = load_checker()
    source = """ARG FEATURES=""
RUN cargo build --all-features --bin iroha3d
"""
    try:
        checker._validate_dockerfile_cargo_scope(source, Path("Dockerfile"))
    except RuntimeError as error:
        assert "hardcoded --all-features" in str(error)
    else:  # pragma: no cover - failure branch
        raise AssertionError("Dockerfile --all-features bypass was accepted")


def test_dockerfile_unknown_cargo_argument_injection_is_rejected(
    tmp_path: Path,
) -> None:
    checker = load_checker()
    workflow_dir = prepare_docker_parser_repo(
        tmp_path,
        """ARG FEATURES=""
ARG CARGOFLAGS=""
ARG BINARIES="demo"
ARG EXTRA_CARGO_ARGS=""
RUN cargo ${EXTRA_CARGO_ARGS} build --features "${FEATURES}" --bin demo
""",
    )
    (workflow_dir / "publish.yml").write_text(
        """jobs:
  release:
    steps:
      - uses: docker/build-push-action@pinned
        with:
          context: .
          build-args: |
            "EXTRA_CARGO_ARGS=--features iroha/test-fixtures"
""",
        encoding="utf-8",
    )
    try:
        checker.docker_publish_invocations(tmp_path)
    except RuntimeError as error:
        assert "unreviewed ARG controls Cargo scope" in str(error)
        assert "EXTRA_CARGO_ARGS" in str(error)
    else:  # pragma: no cover - failure branch
        raise AssertionError("Dockerfile Cargo ARG injection was accepted")


def test_remote_docker_build_context_is_rejected(tmp_path: Path) -> None:
    checker = load_checker()
    workflow_dir = prepare_docker_parser_repo(tmp_path)
    (workflow_dir / "publish.yml").write_text(
        """jobs:
  release:
    steps:
      - uses: docker/build-push-action@pinned
        with:
          context: https://example.invalid/unreviewed.git
""",
        encoding="utf-8",
    )
    try:
        checker.docker_publish_invocations(tmp_path)
    except RuntimeError as error:
        assert "context must be the repository root '.'" in str(error)
    else:  # pragma: no cover - failure branch
        raise AssertionError("remote Docker publication context was accepted")


def test_official_workflow_prebuilt_binary_bypass_is_rejected(
    tmp_path: Path,
) -> None:
    checker = load_checker()
    workflow_dir = prepare_docker_parser_repo(
        tmp_path,
        """ARG FEATURES=""
ARG CARGOFLAGS=""
ARG BINARIES="demo"
ARG USE_PREBUILT="0"
RUN cargo ${CARGOFLAGS} build --features "${FEATURES}" --bin demo
""",
    )
    (workflow_dir / "publish.yml").write_text(
        """jobs:
  release:
    steps:
      - uses: docker/build-push-action@pinned
        with:
          context: .
          build-args: |
            "USE_PREBUILT=1"
""",
        encoding="utf-8",
    )
    try:
        checker.docker_publish_invocations(tmp_path)
    except RuntimeError as error:
        assert "may not override USE_PREBUILT" in str(error)
    else:  # pragma: no cover - failure branch
        raise AssertionError("official workflow prebuilt binary bypass was accepted")


def test_indirect_dockerfile_build_and_overwrite_paths_are_rejected() -> None:
    checker = load_checker()
    reviewed = """ARG FEATURES=""
ARG CARGOFLAGS=""
RUN cargo ${CARGOFLAGS} build --features "${FEATURES}" --bin demo
"""
    bypasses = (
        "RUN ./scripts/rebuild.sh\n",
        " run ./scripts/rebuild.sh\n",
        "\tRuN ./scripts/rebuild.sh\n",
        "RUN $CARGO build --all-features --bin demo\n",
        "RUN cp dist/test-fixture-demo /outbin/demo\n",
        "RUN cp dist/test-fixture-demo ./bins/demo\n",
    )
    for bypass in bypasses:
        try:
            checker._validate_dockerfile_cargo_scope(
                reviewed + bypass, Path("Dockerfile")
            )
        except RuntimeError as error:
            assert "review" in str(error)
        else:  # pragma: no cover - failure branch
            raise AssertionError(f"indirect Docker build bypass was accepted: {bypass}")


def test_runtime_copy_must_come_from_reviewed_cargo_output() -> None:
    checker = load_checker()
    reviewed = """ARG FEATURES=""
ARG CARGOFLAGS=""
RUN cargo ${CARGOFLAGS} build --features "${FEATURES}" --bin demo
"""
    overwrites = (
        "COPY dist/test-fixture-demo /usr/local/bin/demo\n",
        " copy dist/test-fixture-demo /usr/local/bin/demo\n",
        "\tCoPy dist/test-fixture-demo /usr/local/bin/demo\n",
        "COPY --from=unreviewed /tmp/demo /usr/local/bin/demo\n",
    )
    for overwrite in overwrites:
        try:
            checker._validate_dockerfile_cargo_scope(
                reviewed + overwrite, Path("Dockerfile")
            )
        except RuntimeError as error:
            assert "runtime executable COPY" in str(error)
        else:  # pragma: no cover - failure branch
            raise AssertionError(f"runtime binary overwrite was accepted: {overwrite}")


def test_docker_action_yaml_anchor_indirection_is_rejected(
    tmp_path: Path,
) -> None:
    checker = load_checker()
    workflow_dir = tmp_path / ".github" / "workflows"
    workflow_dir.mkdir(parents=True)
    (workflow_dir / "anchored_publisher.yml").write_text(
        """x-publish: &publish
  push: true
jobs:
  publish:
    steps:
      - uses: docker/build-push-action@pinned
        with: *publish
""",
        encoding="utf-8",
    )
    try:
        checker.docker_image_publish_workflows(tmp_path)
    except RuntimeError as error:
        assert "YAML anchors and aliases" in str(error)
    else:  # pragma: no cover - failure branch
        raise AssertionError("Docker action YAML anchor bypass was accepted")


def test_workflow_parser_discovers_alternate_step_indentation(
    tmp_path: Path,
) -> None:
    checker = load_checker()
    workflow_dir = tmp_path / ".github" / "workflows"
    workflow_dir.mkdir(parents=True)
    (workflow_dir / "nested_publisher.yml").write_text(
        """jobs:
    publish:
      steps:
          - id: nested-publisher
            uses: docker/build-push-action@pinned
            with:
              push: true
""",
        encoding="utf-8",
    )
    discovered = set(checker.docker_image_publish_workflows(tmp_path))
    assert Path(".github/workflows/nested_publisher.yml") in discovered
    try:
        checker.validate_docker_publish_workflow_classification(tmp_path)
    except RuntimeError as error:
        assert "unclassified Docker image-publishing workflows" in str(error)
    else:  # pragma: no cover - failure branch
        raise AssertionError("alternate-indentation Docker publisher was accepted")


def test_direct_docker_cli_publishers_are_discovered_and_rejected(
    tmp_path: Path,
) -> None:
    checker = load_checker()
    commands = (
        "docker build -t example.invalid/iroha .\ndocker push example.invalid/iroha",
        "docker buildx build --push -t example.invalid/iroha .",
        "docker buildx bake --push release",
        "docker buildx build --output type=registry,name=example.invalid/iroha .",
        "docker buildx build -o type=registry,name=example.invalid/iroha .",
        "docker buildx bake --set '*.output=type=registry' release",
    )
    for command in commands:
        assert checker._workflow_has_direct_docker_publish(f"run: |\n  {command}")

    workflow_dir = tmp_path / ".github" / "workflows"
    workflow_dir.mkdir(parents=True)
    (workflow_dir / "cli_publisher.yml").write_text(
        """jobs:
  publish:
    steps:
      - run: |
          docker build -t example.invalid/iroha .
          docker push example.invalid/iroha
""",
        encoding="utf-8",
    )
    assert Path(".github/workflows/cli_publisher.yml") in set(
        checker.docker_image_publish_workflows(tmp_path)
    )
    try:
        checker.validate_docker_publish_workflow_classification(tmp_path)
    except RuntimeError as error:
        assert "unclassified Docker image-publishing workflows" in str(error)
    else:  # pragma: no cover - failure branch
        raise AssertionError("direct Docker CLI publisher was accepted")


def test_docker_action_registry_output_is_discovered(tmp_path: Path) -> None:
    checker = load_checker()
    workflow_dir = tmp_path / ".github" / "workflows"
    workflow_dir.mkdir(parents=True)
    (workflow_dir / "registry_output.yml").write_text(
        """jobs:
  publish:
    steps:
      - uses: docker/build-push-action@pinned
        with:
          outputs: type=registry,name=example.invalid/iroha
""",
        encoding="utf-8",
    )
    assert Path(".github/workflows/registry_output.yml") in set(
        checker.docker_image_publish_workflows(tmp_path)
    )


def test_docker_bake_action_requires_explicit_guard_support(
    tmp_path: Path,
) -> None:
    checker = load_checker()
    workflow_dir = tmp_path / ".github" / "workflows"
    workflow_dir.mkdir(parents=True)
    (workflow_dir / "bake_publisher.yml").write_text(
        """jobs:
  publish:
    steps:
      - uses: docker/bake-action@pinned
        with:
          set: '*.output=type=registry'
""",
        encoding="utf-8",
    )
    try:
        checker.docker_image_publish_workflows(tmp_path)
    except RuntimeError as error:
        assert "bake action publication requires explicit guard support" in str(error)
    else:  # pragma: no cover - failure branch
        raise AssertionError("Docker bake action publisher bypass was accepted")


def test_unclassified_docker_image_publisher_is_rejected(tmp_path: Path) -> None:
    checker = load_checker()
    workflow_dir = tmp_path / ".github" / "workflows"
    workflow_dir.mkdir(parents=True)
    (workflow_dir / "new_publisher.yml").write_text(
        """jobs:
  publish:
    steps:
      - id: unreviewed-publisher
        uses: docker/build-push-action@pinned
        with:
          push: ${{ github.ref == 'refs/heads/main' }}
""",
        encoding="utf-8",
    )
    try:
        checker.validate_docker_publish_workflow_classification(tmp_path)
    except RuntimeError as error:
        assert "unclassified Docker image-publishing workflows" in str(error)
        assert ".github/workflows/new_publisher.yml" in str(error)
    else:  # pragma: no cover - failure branch
        raise AssertionError("unclassified Docker publisher was accepted")


def test_opaque_docker_build_arguments_are_rejected() -> None:
    checker = load_checker()
    step = """      - uses: docker/build-push-action@pinned
        with:
          build-args: ${{ matrix.release_build_args }}
"""
    try:
        checker._docker_build_arguments(step, {}, Path("publisher.yml"))
    except RuntimeError as error:
        assert "one reviewable KEY=value per entry" in str(error)
    else:  # pragma: no cover - failure branch
        raise AssertionError("opaque Docker build argument expansion was accepted")


def test_all_image_publishers_are_explicitly_classified() -> None:
    checker = load_checker()
    discovered = set(checker.docker_image_publish_workflows(REPO))
    runtime = set(checker.DOCKER_PUBLISH_WORKFLOWS)
    nonshipping = set(checker.NONSHIPPING_DOCKER_PUBLISH_WORKFLOWS)
    assert discovered <= runtime | nonshipping
    assert Path(".github/workflows/publish_dev.yml") in discovered
    assert checker.NONSHIPPING_DOCKER_PUBLISH_WORKFLOWS[
        Path(".github/workflows/ci_image.yml")
    ]


def test_native_library_roots_and_release_bundle_boundary_are_derived(
    monkeypatch,
) -> None:
    checker = load_checker()
    assert (
        checker.canonical_release_bundle_policy(REPO)
        == "authenticated-prebuilt-reviewed-profile"
    )
    monkeypatch.setattr(checker, "validate_trusted_release_surface", lambda _repo: None)
    targets = checker.declared_shipping_targets(REPO)
    bridge_targets = [
        target
        for target in targets
        if target.package == "connect_norito_bridge"
        and target.binary == "<native-library>"
    ]
    assert bridge_targets
    assert {target.features for target in bridge_targets} >= {
        (),
        ("privacy-production-enabled",),
    }
    assert {
        ".github/workflows/pr_csharp.yml",
        ".github/workflows/mobile_sdk_artifacts.yml",
        ".github/workflows/sorafs-orchestrator-sdk.yml",
        "scripts/build_norito_xcframework.sh",
        "kotlin/client-android/build.gradle.kts",
    } <= {target.source for target in bridge_targets}

    bundle_targets = checker.release_bundle_targets(REPO)
    assert {target.binary for target in bundle_targets} == {
        "iroha3d",
        "sorafs_governance_dag",
        "iroha",
        "kagami",
        "attachment_sanitizer",
        "sorafs_external_software_signer",
    }
    signer = next(
        target
        for target in bundle_targets
        if target.binary == "sorafs_external_software_signer"
    )
    assert "external-software-signer-bin" in signer.features
    assert any(
        target.source == str(checker.RELEASE_BUNDLE_SCRIPT) for target in targets
    )


def test_canonical_release_prebuilt_boundary_fails_closed(tmp_path: Path) -> None:
    checker = load_checker()
    scripts = tmp_path / "scripts"
    scripts.mkdir()
    (tmp_path / checker.ISOLATED_RELEASE_RUNNER).write_text(
        "ALLOWED_TOOLS = frozenset(())\n"
        '"generate_release_manifest.py"\n'
        '"write_release_sha256sums.py"\n'
        '"fastpq/rollout_manifest_summary.py"\n'
        '"verify_release_prebuilt_provenance.py"\n'
        "RELEASE_ARTIFACT_CONTRACT_SHA256\n"
        "REVIEWED_TOOL_SHA256\n"
        "hashlib.sha256(payload).hexdigest()\n"
        "os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW\n"
        "directory_flags = file_flags | os.O_DIRECTORY\n"
        "before.st_nlink != 1\n"
        "stat.S_IWGRP | stat.S_IWOTH\n"
        "os.open(name, file_flags, dir_fd=directory_descriptor)\n"
        "identity(before) != identity(after)\n"
        'exec(compile(payload, str(path), "exec"), module.__dict__)\n'
        'exec(compile(payload, str(path), "exec"), namespace)\n'
        "contract.stable_read_relative(\n"
        "_load_fastpq_summary_dependencies()\n",
        encoding="utf-8",
    )
    verifier_invocation = """prebuilt_provenance_sha256="$(
  "${release_python[@]}" "$repo_root/scripts/verify_release_prebuilt_provenance.py" \\
    --trusted-manifest-sha256 "$digest" \\
    --source-commit "$commit" \\
    --cargo-lock Cargo.lock \\
    --target "$target" \\
    --cargo-profile deploy \\
    --features "$features" \\
    "${provenance_binaries[@]}" \\
    --output-directory "$snapshot"
)"
"""
    isolated_builder_contract = """#!/usr/bin/env -S -u BASH_ENV -u ENV -u SHELLOPTS -u BASHOPTS -u PS4 -u BASH_XTRACEFD -u CDPATH -u GLOBIGNORE bash -p
set -euo pipefail
unset BASH_ENV ENV PYTHONHOME PYTHONPATH PS4 BASH_XTRACEFD CDPATH GLOBIGNORE \\
  CARGO_ENCODED_RUSTFLAGS CARGO_ENCODED_RUSTDOCFLAGS CARGO_HOME \\
  RUSTC RUSTC_WRAPPER RUSTC_WORKSPACE_WRAPPER RUSTDOC RUSTDOCFLAGS RUSTFLAGS
for release_environment_name in ${!CARGO_BUILD_@}; do
  unset "$release_environment_name"
done
for release_environment_name in ${!CARGO_TARGET_@}; do
  case "$release_environment_name" in
    *_LINKER|*_RUNNER|*_RUSTFLAGS|*_RUSTDOCFLAGS) ;;
  esac
done
export PYTHONNOUSERSITE=1
release_python=(python3 -I -S "$repo_root/scripts/run_isolated_release_tool.py")
validate_release_source
validate_release_source
"""
    bundle_snapshot_contract = """printf '%s\\n' '--prebuilt-bin-dir is required for deterministic release bundles'
binary_root=""
binary_root="$stage_parent/prebuilt-bin"
stage_release_file "$binary_root/daemon" out 0755
stage_release_file "$binary_root/dag" out 0755
stage_release_file "$binary_root/cli" out 0755
stage_release_file "$binary_root/utility" out 0755
stage_release_file "$binary_root/sanitizer" out 0755
stage_release_file "$binary_root/signer" out 0755
stage_release_file "$binary_root/signer" broker 0755
"""
    image_snapshot_contract = """prebuilt_bin_dir=""
prebuilt_bin_dir="$2"
prebuilt_snapshot="$temp_root/prebuilt-bin"
prebuilt_bin_dir="$prebuilt_snapshot"
copy_release_file --source "$prebuilt_bin_dir/$binary"
"""
    (tmp_path / checker.RELEASE_BUNDLE_SCRIPT).write_text(
        isolated_builder_contract
        + 'case "$1" in\n  --features) ;;\nesac\n'
        + bundle_snapshot_contract
        + verifier_invocation,
        encoding="utf-8",
    )
    (tmp_path / checker.RELEASE_IMAGE_SCRIPT).write_text(
        isolated_builder_contract + image_snapshot_contract + verifier_invocation,
        encoding="utf-8",
    )
    pipeline = """_BOOTSTRAP_RELEASE_MODULE_SHA256 = {}
os.O_RDONLY | os.O_CLOEXEC | os.O_DIRECTORY | os.O_NOFOLLOW
before.st_nlink != 1
stat.S_IWGRP | stat.S_IWOTH
_normalized_bootstrap_payload(name, bytes(payload))
hashlib.sha256(normalized).hexdigest()
exec(compile(payload, str(path), "exec"), module.__dict__)
_stable_bootstrap_sources()
_HOSTILE_CHILD_ENVIRONMENT = frozenset({"BASH_ENV", "BASHOPTS", "BASH_XTRACEFD", "CDPATH", "ENV", "GLOBIGNORE", "PS4", "PYTHONHOME", "PYTHONPATH", "CARGO_ENCODED_RUSTFLAGS", "CARGO_ENCODED_RUSTDOCFLAGS", "CARGO_HOME", "RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "RUSTDOC", "RUSTDOCFLAGS", "RUSTFLAGS", "SHELLOPTS"})
if name in _HOSTILE_CHILD_ENVIRONMENT or name.startswith(("BASH_FUNC_", "CARGO_BUILD_")):
    environment.pop(name, None)
if name.startswith("CARGO_TARGET_") and name.endswith(("_LINKER", "_RUNNER", "_RUSTFLAGS", "_RUSTDOCFLAGS")):
    environment.pop(name, None)
def validate_release_source(commit: str, action: str) -> None:
    validate_trusted_release_surface_commit(REPO_ROOT, commit)
validate_release_source(commit, "Release source preflight failed")
run_trusted_release_action(commit, "Android Maven publication refused changed release source", lambda: run(publish_cmd, env=release_env))
validate_release_source(commit, "Aggregate manifest signing refused changed release source")
validate_release_source(commit, "Release source changed during pipeline execution")
command = (sys.executable, "-I", "-S", "run_isolated_release_tool.py"),
executable.resolve().is_relative_to(_SCRIPT_DIRECTORY)
path, separator, provenance_sha256 = authenticated_path.rpartition(
            "@sha256:"
        )
bundle_command = [
                    REPO_ROOT / "scripts" / "build_release_bundle.sh",
                    "--prebuilt-bin-dir",
                    bundle_path,
                    "--trusted-prebuilt-provenance-sha256",
                    bundle_digest,
                ]
image_command = [
                    REPO_ROOT / "scripts" / "build_release_image.sh",
                    "--prebuilt-bin-dir",
                    image_path,
                    "--trusted-prebuilt-provenance-sha256",
                    image_digest,
                ]
"""
    (tmp_path / checker.CANONICAL_RELEASE_PIPELINE).write_text(
        pipeline, encoding="utf-8"
    )
    assert (
        checker.canonical_release_bundle_policy(tmp_path)
        == "authenticated-prebuilt-reviewed-profile"
    )

    mutations = (
        (
            pipeline.replace(
                'validate_release_source(commit, "Release source preflight failed")\n',
                "",
                1,
            ),
            "source-commit preflight/recheck changed",
        ),
        (
            pipeline.replace('"BASH_ENV", ', "", 1),
            "hostile subprocess environment scrub changed",
        ),
        (
            pipeline.replace(
                '                    "--trusted-prebuilt-provenance-sha256",\n'
                "                    bundle_digest,\n",
                "",
                1,
            ),
            "prebuilt provenance handoff changed",
        ),
        (
            pipeline.replace('            "@sha256:"', '            "@digest:"'),
            "not parsed as one authenticated identity",
        ),
        (
            pipeline.replace(
                '                    "--prebuilt-bin-dir",',
                '                    "--features",\n'
                "                    dynamic_features,\n"
                '                    "--prebuilt-bin-dir",',
                1,
            ),
            "official bundle may not accept a dynamic feature override",
        ),
        (
            pipeline.replace(
                '                    "--prebuilt-bin-dir",\n'
                "                    image_path,\n",
                '                    "--features",\n'
                "                    dynamic_features,\n"
                '                    "--prebuilt-bin-dir",\n'
                "                    image_path,\n",
                1,
            ),
            "official image may not accept a dynamic feature override",
        ),
    )
    for mutated, message in mutations:
        (tmp_path / checker.CANONICAL_RELEASE_PIPELINE).write_text(
            mutated, encoding="utf-8"
        )
        try:
            checker.canonical_release_bundle_policy(tmp_path)
        except RuntimeError as error:
            assert message in str(error)
        else:  # pragma: no cover - failure branch
            raise AssertionError("unauthenticated canonical prebuilt drift was accepted")

    (tmp_path / checker.CANONICAL_RELEASE_PIPELINE).write_text(
        pipeline, encoding="utf-8"
    )
    for script in (
        checker.RELEASE_BUNDLE_SCRIPT,
        checker.RELEASE_IMAGE_SCRIPT,
    ):
        source = (tmp_path / script).read_text(encoding="utf-8")
        (tmp_path / script).write_text(
            source.replace("verify_release_prebuilt_provenance.py", "unchecked.py"),
            encoding="utf-8",
        )
        try:
            checker.canonical_release_bundle_policy(tmp_path)
        except RuntimeError as error:
            assert "prebuilt provenance verifier contract changed" in str(error)
        else:  # pragma: no cover - failure branch
            raise AssertionError("missing prebuilt verifier was accepted")
        (tmp_path / script).write_text(source, encoding="utf-8")

    bundle_path = tmp_path / checker.RELEASE_BUNDLE_SCRIPT
    bundle_source = bundle_path.read_text(encoding="utf-8")
    bundle_path.write_text(
        bundle_source.replace(
            'binary_root="$stage_parent/prebuilt-bin"',
            'binary_root="$prebuilt_bin_dir"',
        ),
        encoding="utf-8",
    )
    with pytest.raises(RuntimeError, match="private prebuilt snapshot consumption"):
        checker.canonical_release_bundle_policy(tmp_path)
    bundle_path.write_text(bundle_source, encoding="utf-8")

    image_path = tmp_path / checker.RELEASE_IMAGE_SCRIPT
    image_source = image_path.read_text(encoding="utf-8")
    image_path.write_text(
        image_source.replace(
            'prebuilt_bin_dir="$prebuilt_snapshot"',
            'prebuilt_bin_dir="$original_prebuilt_bin_dir"',
        ),
        encoding="utf-8",
    )
    with pytest.raises(RuntimeError, match="private prebuilt snapshot consumption"):
        checker.canonical_release_bundle_policy(tmp_path)
    image_path.write_text(image_source, encoding="utf-8")

    for script in (checker.RELEASE_BUNDLE_SCRIPT, checker.RELEASE_IMAGE_SCRIPT):
        path = tmp_path / script
        source = path.read_text(encoding="utf-8")
        path.write_text(
            source.replace(
                'release_python=(python3 -I -S "$repo_root/scripts/',
                'release_python=(python3 "$repo_root/scripts/',
                1,
            ),
            encoding="utf-8",
        )
        with pytest.raises(RuntimeError, match="isolated release helper launcher"):
            checker.canonical_release_bundle_policy(tmp_path)
        path.write_text(source, encoding="utf-8")

        path.write_text(
            source.replace(
                '"${release_python[@]}" "$repo_root/scripts/'
                'verify_release_prebuilt_provenance.py"',
                'python3 "$repo_root/scripts/verify_release_prebuilt_provenance.py"',
                1,
            ),
            encoding="utf-8",
        )
        with pytest.raises(RuntimeError, match="bypasses isolated launcher"):
            checker.canonical_release_bundle_policy(tmp_path)
        path.write_text(source, encoding="utf-8")


def test_android_gradle_native_build_owner_rejects_feature_scope_drift(
    tmp_path: Path,
) -> None:
    checker = load_checker()
    catalog = checker.WorkspaceCatalog(
        package_features={
            "connect_norito_bridge": frozenset({"privacy-production-enabled"})
        },
        binaries={},
        native_libraries={"connect_norito_bridge": ("cdylib", "staticlib")},
        workspace_docker_bins=(),
    )
    targets = checker.android_native_artifact_targets(REPO, catalog)
    assert {target.features for target in targets} == {()}

    for relative in (
        Path(".github/workflows/mobile_sdk_artifacts.yml"),
        checker.ANDROID_NATIVE_BUILD_OWNER,
        checker.ANDROID_HERMETIC_RUNNER,
    ):
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes((REPO / relative).read_bytes())
    owner = tmp_path / checker.ANDROID_NATIVE_BUILD_OWNER
    source = owner.read_text(encoding="utf-8")
    owner.write_text(
        source.replace(
            'addAll(listOf("--features", "privacy-production-enabled"))',
            'addAll(listOf("--features", "iroha_data_model/test-fixtures"))\n'
            '                    addAll(listOf("--features", '
            '"privacy-production-enabled"))',
            1,
        ),
        encoding="utf-8",
    )
    try:
        checker.android_native_artifact_targets(tmp_path, catalog)
    except RuntimeError as error:
        assert "Android Cargo feature" in str(error)
    else:  # pragma: no cover - failure branch
        raise AssertionError("Android Gradle Cargo feature injection was accepted")


def test_workflow_parser_tracks_continued_package_feature_build() -> None:
    checker = load_checker()
    commands = checker._workflow_build_commands(
        """run: |
          cargo +stable b --locked --release -p sorafs_car \\
            -F cli --bin sorafs_fetch
        """
    )
    assert commands == (
        (
            "cargo",
            "+stable",
            "b",
            "--locked",
            "--release",
            "-p",
            "sorafs_car",
            "-F",
            "cli",
            "--bin",
            "sorafs_fetch",
        ),
    )
    assert checker._option_values(commands[0], "-F", "--features") == ("cli",)
    assert checker._workflow_build_commands(
        "cargo rustc --locked -p connect_norito_bridge --lib --crate-type cdylib"
    ) == (
        (
            "cargo",
            "rustc",
            "--locked",
            "-p",
            "connect_norito_bridge",
            "--lib",
            "--crate-type",
            "cdylib",
        ),
    )


def test_forbidden_feature_detection_rejects_core_test_surface() -> None:
    checker = load_checker()
    graph = 'iroha_core feature "iroha-core-tests"\n'
    assert checker.forbidden_features_in_graph(graph) == (
        'iroha_core feature "iroha-core-tests"',
    )


def test_positive_shipping_feature_policy_rejects_dev_and_test_roots() -> None:
    checker = load_checker()
    allowed = (
        checker.ShippingProfile("irohad", ("daemon",)),
        checker.ShippingProfile("iroha_cli", ("cli",)),
        checker.ShippingProfile(
            "connect_norito_bridge", ("privacy-production-enabled",)
        ),
    )
    checker.validate_shipping_profile_policy(allowed)

    rejected_profiles = (
        checker.ShippingProfile("irohad", ("dev-tools",)),
        checker.ShippingProfile("irohad", ("test-network-message-control",)),
        checker.ShippingProfile("irohad", ("test-network-parliament-signers",)),
        checker.ShippingProfile("iroha_cli", ("cli_integration_harness",)),
        checker.ShippingProfile("new_release_package"),
    )
    for profile in rejected_profiles:
        try:
            checker.validate_shipping_profile_policy((profile,))
        except RuntimeError as error:
            assert "shipping feature policy violations" in str(error)
        else:  # pragma: no cover - failure branch
            raise AssertionError(f"unreviewed shipping profile accepted: {profile}")

    graph = """irohad feature "daemon"
irohad feature "test-network-message-control"
irohad feature "test-network-parliament-signers"
"""
    assert checker.unauthorized_root_features_in_graph(graph, "irohad") == (
        "test-network-message-control",
        "test-network-parliament-signers",
    )


def test_kagami_keeps_core_test_surface_out_of_normal_dependencies() -> None:
    with (REPO / "crates" / "iroha_kagami" / "Cargo.toml").open("rb") as source:
        manifest = tomllib.load(source)
    assert "iroha-core-tests" not in manifest["dependencies"]["iroha_core"]["features"]
    assert "iroha-core-tests" in manifest["dev-dependencies"]["iroha_core"]["features"]


def test_pr_workflow_runs_release_feature_graph_guard() -> None:
    workflow = WORKFLOW.read_text(encoding="utf-8")
    assert "scripts/tests/release_feature_graph_test.py" in workflow
    assert "python3 -I -S scripts/check_release_feature_graph.py" in workflow
    pull_request_trigger = workflow.split("concurrency:", 1)[0]
    assert "paths-ignore:" not in pull_request_trigger


def test_release_publishers_depend_on_feature_graph_guard() -> None:
    guarded_jobs = {
        Path(".github/workflows/publish.yml"): (
            "candidate-no-profiling",
            "candidate-with-profiling",
        ),
        Path(".github/workflows/publish_custom.yml"): ("image",),
        Path(".github/workflows/publish_dev.yml"): ("dev_image",),
        Path(".github/workflows/publish_xx.yml"): ("image",),
    }
    for relative, jobs in guarded_jobs.items():
        workflow = (REPO / relative).read_text(encoding="utf-8")
        assert (
            workflow.count("python3 -I -S scripts/check_release_feature_graph.py")
            == 1
        )
        assert re.search(r"(?m)^  release-feature-graph:\s*$", workflow)
        for job in jobs:
            pattern = (
                rf"(?m)^  {re.escape(job)}:\s*\n"
                r"    needs: release-feature-graph\s*$"
            )
            assert re.search(pattern, workflow), f"{relative}:{job} bypasses guard"

    for relative in (
        Path(".github/workflows/sorafs-cli-release.yml"),
        Path(".github/workflows/mobile_sdk_artifacts.yml"),
    ):
        workflow = (REPO / relative).read_text(encoding="utf-8")
        assert "python3 -I -S scripts/check_release_feature_graph.py" in workflow
