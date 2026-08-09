"""Tests for scripts/check_generated_artifacts.py."""

from __future__ import annotations

import shutil
import subprocess
from pathlib import Path

import pytest

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - Python 3.10 compatibility
    import tomli as tomllib


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "check_generated_artifacts.py"


def _write_manifest(repo: Path, *, extra: str = "") -> None:
    (repo / "generated-files.toml").write_text(
        """
schema_version = 1

[policy]
forbidden_tracked_globs = [
  "dist/**",
  "**/node_modules/**",
  "**/.docusaurus/**",
  "**/__pycache__/**",
]
allowed_tracked_paths = ["dist/.gitkeep"]
generated_source_extensions = [".rs", ".py"]

[[generated]]
name = "demo"
kind = "file"
outputs = ["src/generated.rs"]
generator = "python3 scripts/generate.py"
generator_sources = ["scripts/generate.py"]
inputs = ["spec/*.toml"]
check = "python3 scripts/generate.py --check"
""".lstrip()
        + extra,
        encoding="utf-8",
    )


def _init_repo(tmp_path: Path) -> Path:
    repo = tmp_path / "repo"
    (repo / "src").mkdir(parents=True)
    (repo / "scripts").mkdir()
    (repo / "spec").mkdir()
    shutil.copy2(SCRIPT, repo / "scripts" / "check_generated_artifacts.py")
    (repo / "scripts" / "generate.py").write_text(
        '"""Deterministic test generator."""\n',
        encoding="utf-8",
    )
    (repo / "spec" / "demo.toml").write_text("version = 1\n", encoding="utf-8")
    (repo / "src" / "generated.rs").write_text(
        "// @generated\npub const VALUE: u8 = 1;\n",
        encoding="utf-8",
    )
    _write_manifest(repo)
    subprocess.run(["git", "init", "-q"], cwd=repo, check=True)
    subprocess.run(["git", "add", "."], cwd=repo, check=True)
    return repo


def _run(repo: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            "python3",
            str(repo / "scripts" / "check_generated_artifacts.py"),
            "--root",
            str(repo),
        ],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )


def test_valid_manifest_and_repository_pass(tmp_path: Path) -> None:
    repo = _init_repo(tmp_path)

    result = _run(repo)

    assert result.returncode == 0
    assert "1 outputs" in result.stdout


def test_repository_policy_forbids_dependency_and_portal_caches() -> None:
    manifest = tomllib.loads(
        (ROOT / "generated-files.toml").read_text(encoding="utf-8")
    )

    assert {
        "**/node_modules/**",
        "**/.docusaurus/**",
    } <= set(manifest["policy"]["forbidden_tracked_globs"])


def test_sorafs_por_aggregate_fixtures_have_complete_unique_owner() -> None:
    fixture_root = ROOT / "fixtures" / "sorafs_manifest"
    managed_directories = (
        "governance",
        "moderation",
        "por",
        "potr",
        "reference_sdk",
        "repair",
    )
    expected_outputs = {
        fixture.relative_to(ROOT).as_posix()
        for directory in managed_directories
        for fixture in (fixture_root / directory).rglob("*")
        if fixture.is_file() and fixture.suffix in {".json", ".to"}
    }
    expected_outputs.add(
        "fixtures/sorafs_manifest/reference_sdk_validation_inventory_v1.json"
    )
    assert len(expected_outputs) == 55

    manifest = tomllib.loads(
        (ROOT / "generated-files.toml").read_text(encoding="utf-8")
    )
    owners = [
        entry
        for entry in manifest["generated"]
        if expected_outputs.intersection(entry["outputs"])
    ]

    assert len(owners) == 1
    owner = owners[0]
    assert owner["name"] == "sorafs-por-aggregate-fixtures"
    assert owner["kind"] == "file"
    assert set(owner["outputs"]) == expected_outputs
    assert set(owner["generator_sources"]) == {
        "crates/sorafs_manifest/Cargo.toml",
        "crates/sorafs_manifest/src/bin/generate_por_fixtures.rs",
    }
    assert set(owner["inputs"]) == {
        "fixtures/sorafs_manifest/appeal_finance/**",
        "fixtures/sorafs_manifest/orderbook/**",
        "fixtures/sorafs_manifest/pdp/**",
        "fixtures/sorafs_manifest/provider_admission/**",
        "fixtures/sorafs_manifest/replication_order/**",
    }

    generator = owner["generator"]
    assert "--write" in generator
    assert "--output-dir" in generator
    assert "IROHA_SORAFS_MANIFEST_STAGE" in generator

    check = owner["check"]
    assert "--check" in check
    assert "--output-dir" not in check

    for command in (generator, check):
        assert command.startswith("NORITO_SKIP_BINDINGS_SYNC=1 ")
        assert "--locked" in command
        assert "--offline" in command
        assert "--jobs 1" in command
        assert "-Z unstable-options" in command
        assert "--lockfile-path Cargo.lock" in command
        assert "-p sorafs_manifest" in command
        assert "--features dev-tools" in command
        assert "--bin generate_por_fixtures" in command


def test_current_rust_contract_artifact_has_complete_unique_owner() -> None:
    output = "javascript/iroha_js/test/fixtures/current_rust_contract_artifact.json"
    manifest = tomllib.loads(
        (ROOT / "generated-files.toml").read_text(encoding="utf-8")
    )
    owners = [entry for entry in manifest["generated"] if output in entry["outputs"]]

    assert len(owners) == 1
    owner = owners[0]
    assert owner["name"] == "javascript-current-rust-contract-artifact"
    assert owner["kind"] == "file"
    assert set(owner["generator_sources"]) == {
        "scripts/regenerate_current_rust_contract_artifact.py",
    }
    assert {
        ".cargo/config.toml",
        "Cargo.toml",
        "javascript/iroha_js/test/fixtures/current_rust_contract_artifact.ko",
        "javascript/iroha_js/src/blake2b.js",
        "javascript/iroha_js/src/ivmArtifact.js",
        "javascript/iroha_js/src/kotodamaCompiler/normalize.js",
        "rust-toolchain.toml",
        "crates/**",
        "vendor/**",
    } <= set(owner["inputs"])
    for field, mode in (("generator", "--write"), ("check", "--check")):
        command = owner[field]
        assert mode in command
        assert "--koto" in command
        assert "--git" in command
        assert "--cache-root" in command
        assert "IROHA_KOTODAMA_CACHE_ROOT" in command
        assert "IROHA_GIT" in command
        assert "--ivm-rlib" not in command
        assert "--rustc" not in command


def test_nexus_connect_transfer_fixture_has_closed_unique_staging_owner() -> None:
    output = "fixtures/sdk/nexus_connect_transfer_v1.json"
    manifest = tomllib.loads(
        (ROOT / "generated-files.toml").read_text(encoding="utf-8")
    )
    owners = [entry for entry in manifest["generated"] if output in entry["outputs"]]

    assert len(owners) == 1
    owner = owners[0]
    assert owner["name"] == "nexus-connect-transfer-v1-fixture"
    assert owner["kind"] == "file"
    assert owner["outputs"] == [output]
    assert set(owner["generator_sources"]) == {
        "xtask/src/main.rs",
        "xtask/src/nexus.rs",
    }
    assert set(owner["inputs"]) == {
        "Cargo.toml",
        "crates/iroha/Cargo.toml",
        "crates/iroha/src/nexus_app.rs",
        "crates/iroha_crypto/src/**/*.rs",
        "crates/iroha_data_model/src/**/*.rs",
        "crates/iroha_primitives/src/**/*.rs",
        "crates/norito/src/**/*.rs",
        "rust-toolchain.toml",
        "xtask/Cargo.toml",
    }

    generator = owner["generator"]
    assert "nexus-connect-fixture --write" in generator
    assert "--output-root" in generator
    assert "IROHA_NEXUS_CONNECT_FIXTURE_STAGE" in generator
    assert "$PWD" not in generator
    assert output not in generator

    check = owner["check"]
    assert "nexus-connect-fixture --check" in check
    assert '--output-root \"$PWD\"' in check
    assert "IROHA_NEXUS_CONNECT_FIXTURE_STAGE" not in check

    for command in (generator, check):
        assert command.startswith("cargo run ")
        assert "--locked" in command
        assert "--offline" in command
        assert "--jobs 1" in command
        assert "-Z unstable-options" in command
        assert "--lockfile-path Cargo.lock" in command
        assert "-p xtask" in command
        assert "--features dev-tools" in command
        assert "--bin xtask" in command


def test_kagemusha_peer_payment_fixture_has_complete_unique_replay_owner() -> None:
    output = "crates/connect_norito_bridge/tests/fixtures/offline_peer_payment_v4.hex"
    recipient = (
        "crates/connect_norito_bridge/tests/fixtures/"
        "offline_recipient_payment_request_v2.hex"
    )
    manifest = tomllib.loads(
        (ROOT / "generated-files.toml").read_text(encoding="utf-8")
    )
    owners = [entry for entry in manifest["generated"] if output in entry["outputs"]]

    assert len(owners) == 1
    owner = owners[0]
    assert owner["name"] == "swift-kagemusha-peer-payment-v4-fixture"
    assert owner["kind"] == "file"
    assert set(owner["generator_sources"]) == {
        "tools/kotlin-fixture-gen/Cargo.toml",
        "tools/kotlin-fixture-gen/src/bin/swift_kagemusha_peer_payment_v4.rs",
    }
    assert {
        recipient,
        "crates/iroha_crypto/src/**/*.rs",
        "crates/iroha_data_model/src/**/*.rs",
        "crates/iroha_primitives/src/**/*.rs",
        "crates/norito/src/**/*.rs",
    } <= set(owner["inputs"])

    generator = owner["generator"]
    assert "--recipient-request-hex" in generator
    assert recipient in generator
    assert "--output" in generator
    assert "IROHA_KAGEMUSHA_PEER_PAYMENT_V4_STAGE" in generator
    assert output not in generator

    check = owner["check"]
    assert "--recipient-request-hex" in check
    assert recipient in check
    assert "--check" in check
    assert output in check

    for command in (generator, check):
        assert "--locked" in command
        assert "--offline" in command
        assert "--jobs 1" in command
        assert "--lockfile-path Cargo.lock" in command
        assert "--bin swift_kagemusha_peer_payment_v4" in command


def test_single_star_input_does_not_cross_directory_boundary(tmp_path: Path) -> None:
    repo = _init_repo(tmp_path)
    subprocess.run(["git", "rm", "-f", "spec/demo.toml"], cwd=repo, check=True)
    nested = repo / "spec" / "nested" / "demo.toml"
    nested.parent.mkdir(parents=True)
    nested.write_text("version = 1\n", encoding="utf-8")
    subprocess.run(["git", "add", str(nested)], cwd=repo, check=True)

    result = _run(repo)

    assert result.returncode == 1
    assert "inputs pattern matches no tracked regular files" in result.stderr


@pytest.mark.parametrize("nested", [False, True])
def test_globstar_input_matches_zero_or_more_directories(
    tmp_path: Path, nested: bool
) -> None:
    repo = _init_repo(tmp_path)
    manifest = repo / "generated-files.toml"
    manifest.write_text(
        manifest.read_text(encoding="utf-8").replace(
            'inputs = ["spec/*.toml"]',
            'inputs = ["spec/**/*.toml"]',
        ),
        encoding="utf-8",
    )
    if nested:
        subprocess.run(["git", "rm", "-f", "spec/demo.toml"], cwd=repo, check=True)
        fixture = repo / "spec" / "nested" / "demo.toml"
        fixture.parent.mkdir(parents=True)
        fixture.write_text("version = 1\n", encoding="utf-8")
        subprocess.run(["git", "add", str(fixture)], cwd=repo, check=True)
    subprocess.run(["git", "add", str(manifest)], cwd=repo, check=True)

    result = _run(repo)

    assert result.returncode == 0


@pytest.mark.parametrize(
    "relative_path",
    (
        "dist/bundle.js",
        "docs/portal/node_modules/package/index.js",
        "docs/portal/.docusaurus/cache.json",
    ),
)
def test_forbidden_tracked_build_artifact_fails(
    tmp_path: Path, relative_path: str
) -> None:
    repo = _init_repo(tmp_path)
    path = repo / relative_path
    path.parent.mkdir(parents=True)
    path.write_text("generated package output\n", encoding="utf-8")
    subprocess.run(["git", "add", str(path)], cwd=repo, check=True)

    result = _run(repo)

    assert result.returncode == 1
    assert "forbidden generated/build artifacts" in result.stderr
    assert relative_path in result.stderr


def test_forbidden_tracked_docusaurus_cache_fails(tmp_path: Path) -> None:
    repo = _init_repo(tmp_path)
    path = repo / "docs" / "portal" / ".docusaurus" / "client-manifest.json"
    path.parent.mkdir(parents=True)
    path.write_text("{}\n", encoding="utf-8")
    subprocess.run(["git", "add", str(path)], cwd=repo, check=True)

    result = _run(repo)

    assert result.returncode == 1
    assert "forbidden generated/build artifacts" in result.stderr
    assert "docs/portal/.docusaurus/client-manifest.json" in result.stderr


def test_allowed_directory_marker_passes(tmp_path: Path) -> None:
    repo = _init_repo(tmp_path)
    path = repo / "dist" / ".gitkeep"
    path.parent.mkdir()
    path.write_text("", encoding="utf-8")
    subprocess.run(["git", "add", "-f", str(path)], cwd=repo, check=True)

    result = _run(repo)

    assert result.returncode == 0


def test_unregistered_generated_header_fails(tmp_path: Path) -> None:
    repo = _init_repo(tmp_path)
    path = repo / "src" / "orphan.rs"
    path.write_text("// Code generated by demo; DO NOT EDIT.\n", encoding="utf-8")
    subprocess.run(["git", "add", str(path)], cwd=repo, check=True)

    result = _run(repo)

    assert result.returncode == 1
    assert "generated source has no generated-files.toml owner" in result.stderr
    assert "src/orphan.rs" in result.stderr


def test_duplicate_output_ownership_fails(tmp_path: Path) -> None:
    repo = _init_repo(tmp_path)
    with (repo / "generated-files.toml").open("a", encoding="utf-8") as manifest:
        manifest.write(
            """

[[generated]]
name = "duplicate"
kind = "regions"
outputs = ["src/generated.rs"]
generator = "python3 scripts/generate.py"
generator_sources = ["scripts/generate.py"]
inputs = ["spec/demo.toml"]
check = "python3 scripts/generate.py --check"
"""
        )
    subprocess.run(["git", "add", "generated-files.toml"], cwd=repo, check=True)

    result = _run(repo)

    assert result.returncode == 1
    assert "owned by both demo and duplicate" in result.stderr


def test_missing_generator_source_fails(tmp_path: Path) -> None:
    repo = _init_repo(tmp_path)
    manifest = repo / "generated-files.toml"
    manifest.write_text(
        manifest.read_text(encoding="utf-8").replace(
            'generator_sources = ["scripts/generate.py"]',
            'generator_sources = ["scripts/missing.py"]',
        ),
        encoding="utf-8",
    )
    subprocess.run(["git", "add", "generated-files.toml"], cwd=repo, check=True)

    result = _run(repo)

    assert result.returncode == 1
    assert "generator_sources is not a tracked regular file" in result.stderr


def test_tracked_elf_core_dump_fails(tmp_path: Path) -> None:
    repo = _init_repo(tmp_path)
    path = repo / "crates" / "demo" / "core"
    path.parent.mkdir(parents=True)
    path.write_bytes(b"\x7fELF" + b"\0" * 32)
    subprocess.run(["git", "add", str(path)], cwd=repo, check=True)

    result = _run(repo)

    assert result.returncode == 1
    assert "executable core dumps are tracked" in result.stderr
    assert "crates/demo/core" in result.stderr


@pytest.mark.parametrize("invalid", ["../outside.rs", "/absolute.rs", "src//bad.rs"])
def test_non_normalized_output_path_fails(tmp_path: Path, invalid: str) -> None:
    repo = _init_repo(tmp_path)
    manifest = repo / "generated-files.toml"
    manifest.write_text(
        manifest.read_text(encoding="utf-8").replace("src/generated.rs", invalid),
        encoding="utf-8",
    )
    subprocess.run(["git", "add", "generated-files.toml"], cwd=repo, check=True)

    result = _run(repo)

    assert result.returncode == 1
    assert "normalized repository-relative path" in result.stderr
