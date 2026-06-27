"""Regression tests for the NoritoBridge XCFramework build fallback."""

from __future__ import annotations

import os
import plistlib
import stat
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "build_norito_xcframework.sh"
STATIC_LIB_NAME = "libconnect_norito_bridge.a"


def _write_executable(path: Path, contents: str) -> None:
    path.write_text(contents, encoding="utf-8")
    path.chmod(path.stat().st_mode | stat.S_IXUSR)


def _write_static_library(build_dir: Path, triple: str, contents: str) -> None:
    library = (
        build_dir
        / "cargo-ios15_0-sim15_0"
        / triple
        / triple
        / "release"
        / STATIC_LIB_NAME
    )
    library.parent.mkdir(parents=True, exist_ok=True)
    library.write_text(contents, encoding="utf-8")


def test_manual_xcframework_fallback_writes_required_info_plist(
    tmp_path: Path,
) -> None:
    build_dir = tmp_path / "build"
    out_dir = tmp_path / "dist"
    tools_dir = tmp_path / "tools"
    tools_dir.mkdir()

    _write_static_library(build_dir, "aarch64-apple-ios", "device")
    _write_static_library(build_dir, "aarch64-apple-ios-sim", "sim-arm")
    _write_static_library(build_dir, "x86_64-apple-ios", "sim-x64")
    _write_static_library(build_dir, "aarch64-apple-darwin", "macos")

    _write_executable(
        tools_dir / "lipo",
        """#!/usr/bin/env bash
set -euo pipefail
output=""
inputs=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    -create)
      ;;
    -output)
      shift
      output="$1"
      ;;
    *)
      inputs+=("$1")
      ;;
  esac
  shift
done
mkdir -p "$(dirname "$output")"
: >"$output"
for input in "${inputs[@]}"; do
  cat "$input" >>"$output"
done
""",
    )
    _write_executable(
        tools_dir / "xcodebuild",
        """#!/usr/bin/env bash
exit 65
""",
    )
    _write_executable(
        tools_dir / "shasum",
        """#!/usr/bin/env bash
last=""
for arg in "$@"; do
  last="$arg"
done
printf '%064d  %s\\n' 0 "$last"
""",
    )

    env = os.environ.copy()
    env.update(
        {
            "NORITO_BRIDGE_BUILD_DIR": str(build_dir),
            "NORITO_BRIDGE_OUT_DIR": str(out_dir),
            "NORITO_BRIDGE_SKIP_CARGO_BUILDS": "1",
            "PATH": f"{tools_dir}{os.pathsep}{env['PATH']}",
        }
    )

    result = subprocess.run(
        ["bash", str(SCRIPT), "--bridge-version", "1.0.0"],
        cwd=ROOT,
        env=env,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )

    assert result.returncode == 0, result.stderr

    xcframework = out_dir / "NoritoBridge.xcframework"
    info = plistlib.loads((xcframework / "Info.plist").read_bytes())
    assert info["XCFrameworkFormatVersion"] == "1.0"
    assert info["CFBundlePackageType"] == "XFWK"

    libraries = {
        entry["LibraryIdentifier"]: entry for entry in info["AvailableLibraries"]
    }
    assert set(libraries) == {
        "ios-arm64",
        "ios-arm64_x86_64-simulator",
        "macos-arm64",
    }

    for identifier, library in libraries.items():
        assert library["LibraryPath"] == "libNoritoBridge.a"
        assert library["HeadersPath"] == "Headers"
        assert (xcframework / identifier / "libNoritoBridge.a").is_file()
        assert (xcframework / identifier / "Headers" / "NoritoBridge.h").is_file()
        assert (
            xcframework / identifier / "Headers" / "connect_norito_bridge.h"
        ).is_file()

    assert libraries["ios-arm64"]["SupportedPlatform"] == "ios"
    assert libraries["ios-arm64"]["SupportedArchitectures"] == ["arm64"]
    assert libraries["ios-arm64_x86_64-simulator"]["SupportedPlatform"] == "ios"
    assert libraries["ios-arm64_x86_64-simulator"][
        "SupportedPlatformVariant"
    ] == "simulator"
    assert libraries["ios-arm64_x86_64-simulator"]["SupportedArchitectures"] == [
        "arm64",
        "x86_64",
    ]
    assert libraries["macos-arm64"]["SupportedPlatform"] == "macos"
    assert libraries["macos-arm64"]["SupportedArchitectures"] == ["arm64"]
