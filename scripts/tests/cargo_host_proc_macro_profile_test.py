"""Keep Cargo's local host proc macros loadable under the actual workspace profiles."""

from __future__ import annotations

import copy
import json
import os
import shutil
import subprocess
from pathlib import Path
try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib

import pytest


MANIFEST = Path(__file__).resolve().parents[2] / "Cargo.toml"


def _check_host_profiles(manifest: dict) -> None:
    profiles = manifest["profile"]
    for name in ("dev", "test", "release"):
        assert profiles[name]["build-override"].get("strip") == "none", (
            f"{name} host proc macros must retain loadable Mach-O metadata"
        )
    assert profiles["dev"]["build-override"] == profiles["test"]["build-override"], (
        "dev/test host profiles must share the same compilation settings"
    )
    for name in ("dev", "test"):
        assert profiles[name]["package"]["*"].get("strip") == "none", (
            f"{name} wildcard package override must retain loadable host proc macros"
        )
    assert profiles["dev"]["package"]["*"] == profiles["test"]["package"]["*"], (
        "dev/test wildcard profiles must share the same compilation settings"
    )


def _manifest() -> dict:
    return tomllib.loads(MANIFEST.read_text(encoding="utf-8"))


def test_actual_workspace_keeps_matching_loadable_host_profiles() -> None:
    _check_host_profiles(_manifest())


def test_local_build_and_test_profiles_reuse_incremental_dependencies() -> None:
    profiles = _manifest()["profile"]
    for name in ("dev", "test"):
        assert profiles[name]["incremental"] is True
    for field in ("debug", "split-debuginfo"):
        assert profiles["dev"][field] == profiles["test"][field]
    for package in profiles["test"]["package"]:
        assert profiles["dev"]["package"][package] == profiles["test"]["package"][package]


def test_real_cargo_reuses_a_dev_dependency_during_test_compilation(tmp_path: Path) -> None:
    """Exercise Cargo freshness with the actual workspace's local profile settings."""
    cargo = shutil.which("cargo")
    if cargo is None:
        pytest.skip("Cargo is required for the real profile reuse check")
    profiles = _manifest()["profile"]
    source = '[workspace]\nresolver = "2"\nmembers = ["dependency", "consumer"]\n'
    for name in ("dev", "test"):
        for suffix, settings in (
            ("", profiles[name]),
            (".build-override", profiles[name]["build-override"]),
            ('.package."*"', profiles[name]["package"]["*"]),
        ):
            source += f"\n[profile.{name}{suffix}]\n"
            source += "".join(f"{key} = {json.dumps(value)}\n" for key, value in settings.items()
                              if not isinstance(value, dict))
    (tmp_path / "Cargo.toml").write_text(source)
    shutil.copyfile(MANIFEST.with_name("rust-toolchain.toml"), tmp_path / "rust-toolchain.toml")
    for name in ("dependency", "consumer"):
        directory = tmp_path / name
        (directory / "src").mkdir(parents=True)
        manifest = f'[package]\nname = "warm_{name}"\nversion = "0.0.0"\nedition = "2024"\n'
        if name == "dependency":
            rust = "//! A dependency for the profile reuse test.\npub fn answer() -> u32 { 42 }\n"
            filename = "lib.rs"
        else:
            manifest += '[dependencies]\nwarm_dependency = { path = "../dependency" }\n'
            rust = "//! A consumer for the profile reuse test.\nfn main() { assert_eq!(warm_dependency::answer(), 42); }\n"
            filename = "main.rs"
        (directory / "Cargo.toml").write_text(manifest)
        (directory / "src" / filename).write_text(rust)
    environment = {key: value for key, value in os.environ.items()
                   if not key.startswith("CARGO_") and key not in {
                       "RUSTFLAGS", "RUSTDOCFLAGS", "RUSTC", "RUSTDOC", "RUSTC_WRAPPER",
                       "RUSTC_WORKSPACE_WRAPPER", "RUSTUP_TOOLCHAIN",
                   }}
    environment["CARGO_HOME"] = str(tmp_path / "cargo-home")
    for args, fresh in ((["build"], False), (["test", "--no-run"], True),
                        (["test", "--no-run"], True)):
        result = subprocess.run([cargo, *args, "--offline", "-p", "warm_consumer",
                                 "--message-format=json"], cwd=tmp_path, env=environment,
                                text=True, capture_output=True, check=True)
        artifacts = [json.loads(line) for line in result.stdout.splitlines()]
        dependency, = [item for item in artifacts if item.get("reason") == "compiler-artifact"
                       and item["target"]["name"] == "warm_dependency"]
        assert dependency["fresh"] is fresh, result.stderr
    assert any((tmp_path / "target/debug/incremental").iterdir())


@pytest.mark.parametrize("name", ("dev", "test", "release"))
@pytest.mark.parametrize("strip", (None, "debuginfo", "symbols"))
def test_each_host_profile_rejects_implicit_or_explicit_stripping(
    name: str, strip: str | None
) -> None:
    manifest = copy.deepcopy(_manifest())
    host = manifest["profile"][name]["build-override"]
    if strip is None:
        del host["strip"]
        # A package/profile setting is insufficient: proc macros are host units.
        manifest["profile"][name]["strip"] = "none"
    else:
        host["strip"] = strip
    with pytest.raises(AssertionError, match=f"{name} host proc macros"):
        _check_host_profiles(manifest)


@pytest.mark.parametrize("field", ("debug", "codegen-units"))
def test_host_profile_drift_cannot_split_the_warm_dev_test_graph(field: str) -> None:
    manifest = copy.deepcopy(_manifest())
    host = manifest["profile"]["test"]["build-override"]
    host[field] += 1
    with pytest.raises(AssertionError, match="dev/test host profiles"):
        _check_host_profiles(manifest)


@pytest.mark.parametrize("name", ("dev", "test"))
@pytest.mark.parametrize("strip", (None, "debuginfo", "symbols"))
def test_wildcard_precedence_cannot_restore_unsafe_host_stripping(
    name: str, strip: str | None
) -> None:
    manifest = copy.deepcopy(_manifest())
    package = manifest["profile"][name]["package"]["*"]
    if strip is None:
        del package["strip"]
    else:
        package["strip"] = strip
    with pytest.raises(AssertionError, match=f"{name} wildcard package override"):
        _check_host_profiles(manifest)


@pytest.mark.parametrize("field", ("debug", "codegen-units"))
def test_wildcard_profile_drift_cannot_split_the_warm_dev_test_graph(field: str) -> None:
    manifest = copy.deepcopy(_manifest())
    package = manifest["profile"]["test"]["package"]["*"]
    package[field] += 1
    with pytest.raises(AssertionError, match="dev/test wildcard profiles"):
        _check_host_profiles(manifest)
