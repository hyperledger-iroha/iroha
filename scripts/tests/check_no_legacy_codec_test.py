"""Tests for scripts/check_no_legacy_codec.sh."""

from __future__ import annotations

import shutil
import subprocess
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "check_no_legacy_codec.sh"
RETIRED_HYPHEN_DEP = "parity" + "-" + "scale" + "-codec"
RETIRED_UNDERSCORE_DEP = "parity" + "_" + "scale" + "_codec"
RETIRED_NATIVE_AMX_V1_SOURCES = (
    f'const RETIRED_WIRE_TOKEN: &str = "{"NativeAmxAttestationBody" + "V1"}";\n',
    f'const RETIRED_WIRE_TOKEN: &str = "{"NativeAmxAttestationQc" + "V1"}";\n',
    f'pub struct {"NativeAmxLeg" + "Record"} {{}}\n',
    f'impl_decode_from_slice_via_codec!({"NativeAmxLeg" + "Record"});\n',
    f'const RETIRED_WIRE_TOKEN: &str = "{"iroha:native-amx:" + "v1"}";\n',
)
RETIRED_LANE_HANDOFF_SOURCES = (
    f"pub struct {'LaneExecutablePayload' + 'HandoffV1'} {{}}\n",
    f"struct {'LaneExecutablePayload' + 'HandoffSignaturePreimage'} {{}}\n",
    f"const {'LANE_EXECUTABLE_PAYLOAD_' + 'HANDOFF_VERSION_V2'}: u8 = 2;\n",
    f'const RETIRED_DOMAIN: &str = "{"nexus:lane-executable-payload-" + "handoff:v2"}";\n',
)


def _init_repo(tmp_path: Path) -> Path:
    repo = tmp_path / "repo"
    repo.mkdir()
    shutil.copy2(SCRIPT, repo / "check_no_legacy_codec.sh")
    subprocess.run(["git", "init", "-q"], cwd=repo, check=True)
    (repo / "Cargo.toml").write_text(
        f"""
[workspace]
members = ["crates/demo"]
resolver = "2"
""".lstrip(),
        encoding="utf-8",
    )
    crate_dir = repo / "crates" / "demo"
    crate_dir.mkdir(parents=True)
    (crate_dir / "Cargo.toml").write_text(
        """
[package]
name = "demo"
version = "0.1.0"
edition = "2024"

[dependencies]
norito = "0.1"
""".lstrip(),
        encoding="utf-8",
    )
    return repo


def _run_guard(repo: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["bash", "check_no_legacy_codec.sh"],
        cwd=repo,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def test_guard_allows_clean_root_and_crate_manifests(tmp_path: Path) -> None:
    repo = _init_repo(tmp_path)

    result = _run_guard(repo)

    assert result.returncode == 0
    assert "No retired codec dependencies found." in result.stdout
    assert "No retired Native AMX V1 consensus codecs found." in result.stdout
    assert "No retired lane executable payload handoff codecs found." in result.stdout


def test_guard_rejects_root_manifest_dependency(tmp_path: Path) -> None:
    repo = _init_repo(tmp_path)
    (repo / "Cargo.toml").write_text(
        f"""
[workspace]
members = ["crates/demo"]
resolver = "2"

[workspace.dependencies]
{RETIRED_HYPHEN_DEP} = "3"
""".lstrip(),
        encoding="utf-8",
    )

    result = _run_guard(repo)

    assert result.returncode == 1
    assert "retired codec dependency detected in:" in result.stderr
    assert str(repo / "Cargo.toml") in result.stderr


def test_guard_rejects_root_manifest_renamed_package_dependency(tmp_path: Path) -> None:
    repo = _init_repo(tmp_path)
    (repo / "Cargo.toml").write_text(
        f"""
[workspace]
members = ["crates/demo"]
resolver = "2"

[workspace.dependencies]
legacy_codec = {{ package = "{RETIRED_HYPHEN_DEP}", version = "3" }}
""".lstrip(),
        encoding="utf-8",
    )

    result = _run_guard(repo)

    assert result.returncode == 1
    assert "retired codec dependency detected in:" in result.stderr
    assert str(repo / "Cargo.toml") in result.stderr


def test_guard_rejects_nested_crate_manifest_dependency(tmp_path: Path) -> None:
    repo = _init_repo(tmp_path)
    manifest = repo / "crates" / "demo" / "Cargo.toml"
    manifest.write_text(
        f"""
[package]
name = "demo"
version = "0.1.0"
edition = "2024"

[dependencies]
{RETIRED_UNDERSCORE_DEP} = "3"
""".lstrip(),
        encoding="utf-8",
    )

    result = _run_guard(repo)

    assert result.returncode == 1
    assert "retired codec dependency detected in:" in result.stderr
    assert str(manifest) in result.stderr


def test_guard_rejects_nested_renamed_package_dependency(tmp_path: Path) -> None:
    repo = _init_repo(tmp_path)
    manifest = repo / "crates" / "demo" / "Cargo.toml"
    manifest.write_text(
        f"""
[package]
name = "demo"
version = "0.1.0"
edition = "2024"

[dev-dependencies]
legacy_codec = {{ package = "{RETIRED_HYPHEN_DEP}", version = "3" }}
""".lstrip(),
        encoding="utf-8",
    )

    result = _run_guard(repo)

    assert result.returncode == 1
    assert "retired codec dependency detected in:" in result.stderr
    assert str(manifest) in result.stderr


@pytest.mark.parametrize("retired_source", RETIRED_NATIVE_AMX_V1_SOURCES)
def test_guard_rejects_retired_native_amx_v1_consensus_codec(
    tmp_path: Path, retired_source: str
) -> None:
    repo = _init_repo(tmp_path)
    source = repo / "crates" / "demo" / "src" / "lib.rs"
    source.parent.mkdir()
    source.write_text(retired_source, encoding="utf-8")

    result = _run_guard(repo)

    assert result.returncode == 1
    assert "retired Native AMX V1 consensus codec detected in:" in result.stderr
    assert str(source) in result.stderr


@pytest.mark.parametrize("retired_source", RETIRED_LANE_HANDOFF_SOURCES)
def test_guard_rejects_retired_lane_executable_payload_handoff_codec(
    tmp_path: Path, retired_source: str
) -> None:
    repo = _init_repo(tmp_path)
    source = repo / "crates" / "demo" / "src" / "lib.rs"
    source.parent.mkdir()
    source.write_text(retired_source, encoding="utf-8")

    result = _run_guard(repo)

    assert result.returncode == 1
    assert "retired lane executable payload handoff codec detected in:" in result.stderr
    assert str(source) in result.stderr
