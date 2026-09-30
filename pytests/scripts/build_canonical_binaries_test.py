"""Canonical binary builds preserve the daemon's shipping feature defaults."""

from __future__ import annotations

import os
import subprocess
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[2]
BUILDER = ROOT / "scripts" / "build_canonical_binaries.sh"


@pytest.mark.parametrize("profile", [None, "debug", "deploy"])
@pytest.mark.parametrize("host_target", ["aarch64-apple-darwin", "x86_64-unknown-linux-gnu"])
def test_canonical_build_keeps_default_acceleration_features(
    tmp_path: Path, profile: str | None, host_target: str
) -> None:
    cargo = tmp_path / "cargo"
    cargo.write_text('#!/bin/sh\nprintf "%s\\n" "$@" > "$CAPTURE_CARGO_ARGS"\n')
    cargo.chmod(0o755)
    rustc = tmp_path / "rustc"
    rustc.write_text('#!/bin/sh\nprintf "host: %s\\n" "$CAPTURE_HOST_TARGET"\n')
    rustc.chmod(0o755)
    capture = tmp_path / "cargo-args.txt"
    environment = os.environ.copy()
    environment["PATH"] = f"{tmp_path}{os.pathsep}{environment['PATH']}"
    environment["CAPTURE_CARGO_ARGS"] = str(capture)
    environment["CAPTURE_HOST_TARGET"] = host_target
    environment.pop("IVM_CUDA_TRUSTED_KEY_SHA256", None)
    if profile == "deploy" and "-linux-" in host_target:
        environment["IVM_CUDA_TRUSTED_KEY_SHA256"] = "a" * 64
    if profile is None:
        environment.pop("BUILD_PROFILE", None)
    else:
        environment["BUILD_PROFILE"] = profile

    subprocess.run(
        ["bash", str(BUILDER)],
        cwd=ROOT,
        env=environment,
        check=True,
        capture_output=True,
        text=True,
    )
    arguments = capture.read_text().splitlines()
    assert arguments[:2] == ["build", "--locked"]
    if profile is None:
        assert "--profile" not in arguments
    else:
        assert arguments[arguments.index("--profile") + 1] == profile
    assert arguments[arguments.index("--target") + 1] == host_target
    assert "--no-default-features" not in arguments
    features = "irohad/external-software-signer-bin,iroha_cli/cli"
    if profile == "deploy" and "-linux-" in host_target:
        features += ",irohad/ivm-cuda"
    assert arguments[arguments.index("--features") + 1] == features
    assert arguments[-8:] == [
        "--bin", "iroha3d",
        "--bin", "sorafs_governance_dag",
        "--bin", "sorafs_external_software_signer",
        "--bin", "iroha",
    ]
