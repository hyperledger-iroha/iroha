"""Canonical binary builds preserve the daemon's shipping feature defaults."""

from __future__ import annotations

import os
import subprocess
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[2]
BUILDER = ROOT / "scripts" / "build_canonical_binaries.sh"


@pytest.mark.parametrize("profile", [None, "deploy"])
@pytest.mark.parametrize("host_os", ["Darwin", "Linux"])
def test_canonical_build_keeps_default_acceleration_features(
    tmp_path: Path, profile: str | None, host_os: str
) -> None:
    cargo = tmp_path / "cargo"
    cargo.write_text('#!/bin/sh\nprintf "%s\\n" "$@" > "$CAPTURE_CARGO_ARGS"\n')
    cargo.chmod(0o755)
    uname = tmp_path / "uname"
    uname.write_text('#!/bin/sh\nprintf "%s\\n" "$CAPTURE_HOST_OS"\n')
    uname.chmod(0o755)
    capture = tmp_path / "cargo-args.txt"
    environment = os.environ.copy()
    environment["PATH"] = f"{tmp_path}{os.pathsep}{environment['PATH']}"
    environment["CAPTURE_CARGO_ARGS"] = str(capture)
    environment["CAPTURE_HOST_OS"] = host_os
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
    assert (arguments[2:4] == ["--profile", "deploy"]) == (profile == "deploy")
    assert "--no-default-features" not in arguments
    features = "irohad/external-software-signer-bin,iroha_cli/cli"
    if host_os == "Linux":
        features += ",irohad/ivm-cuda"
    assert arguments[arguments.index("--features") + 1] == features
    assert arguments[-8:] == [
        "--bin", "iroha3d",
        "--bin", "sorafs_governance_dag",
        "--bin", "sorafs_external_software_signer",
        "--bin", "iroha",
    ]
