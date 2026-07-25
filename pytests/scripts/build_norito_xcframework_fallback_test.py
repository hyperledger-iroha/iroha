"""Regression tests for the NoritoBridge XCFramework build fallback."""

from __future__ import annotations

import hashlib
import json
import os
import plistlib
import shutil
import signal
import stat
import subprocess
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "build_norito_xcframework.sh"
STATIC_LIB_NAME = "libconnect_norito_bridge.a"
PUBLIC_STATIC_LIB_NAME = "libNoritoBridge.a"
SLICE_IDS = (
    "ios-arm64",
    "ios-arm64_x86_64-simulator",
    "macos-arm64",
)
REPO_BUILD_LOCK = ROOT / "build" / ".NoritoBridge.build-publish.lockfile"


def _write_executable(path: Path, contents: str) -> None:
    path.write_text(contents, encoding="utf-8")
    path.chmod(path.stat().st_mode | stat.S_IXUSR)


def _write_static_library(build_dir: Path, triple: str, contents: str) -> None:
    library = (
        build_dir
        / "cargo-ios15_0-sim15_0-privacy-production-disabled"
        / triple
        / triple
        / "release"
        / STATIC_LIB_NAME
    )
    library.parent.mkdir(parents=True, exist_ok=True)
    library.write_text(contents, encoding="utf-8")


def _write_live_artifact(out_dir: Path) -> tuple[Path, bytes]:
    framework = out_dir / "NoritoBridge.xcframework"
    framework.mkdir(parents=True)
    (framework / "live-sentinel").write_text(
        "complete-old-artifact\n",
        encoding="utf-8",
    )
    hashes = {}
    for identifier in SLICE_IDS:
        binary = framework / identifier / PUBLIC_STATIC_LIB_NAME
        binary.parent.mkdir(parents=True)
        contents = f"old-{identifier}\n".encode()
        binary.write_bytes(contents)
        hashes[identifier] = hashlib.sha256(contents).hexdigest()
    manifest_bytes = (
        json.dumps(
            {
                "native_bridge_abi_version": 19,
                "hashes": hashes,
            },
            sort_keys=True,
        )
        + "\n"
    ).encode()
    (out_dir / "NoritoBridge.artifacts.json").write_bytes(manifest_bytes)
    return framework, manifest_bytes


def _write_fake_tools(tools_dir: Path) -> None:
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
set -euo pipefail
candidate=""
while [[ $# -gt 0 ]]; do
  if [[ "$1" == "-output" ]]; then
    shift
    candidate="$1"
  fi
  shift
done
test -n "$candidate"
test -f "${NORITO_BRIDGE_OUT_DIR:?}/NoritoBridge.xcframework/live-sentinel"
if [[ -n "${NORITO_BRIDGE_XCODE_WAIT_MARKER:-}" ]]; then
  : > "$NORITO_BRIDGE_XCODE_WAIT_MARKER"
  while true; do
    sleep 1
  done
fi
mkdir -p "$candidate/unexpected-partial"
printf 'must-not-publish\\n' > "$candidate/unexpected-partial/junk"
exit 65
""",
    )
    _write_executable(
        tools_dir / "python3",
        """#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == "-" ]]; then
  exec "${REAL_PYTHON3:?}" "$@"
fi
if [[ "${1:-}" == */exec_with_file_lock.py ]]; then
  exec "${REAL_PYTHON3:?}" "$@"
fi
case "${2:-}" in
  fingerprint)
    printf 'fallback-test-source-fingerprint\\n'
    ;;
  status)
    printf 'fallback-test-dirty-source\\n'
    ;;
  *)
    exit 97
    ;;
esac
""",
    )
    _write_executable(
        tools_dir / "bash",
        """#!/bin/bash
set -euo pipefail
if [[ "${1:-}" == */check_mobile_sdk_artifacts.sh ]]; then
  candidate="${MOBILE_SDK_APPLE_ARTIFACT_DIR:?}"
  test "$candidate" != "${NORITO_BRIDGE_OUT_DIR:?}"
  test -d "$candidate/NoritoBridge.xcframework"
  test -L "$candidate/NoritoBridge.artifacts.json"
  test "$(readlink "$candidate/NoritoBridge.artifacts.json")" = \
    "NoritoBridge.xcframework/NoritoBridge.artifacts.json"
  test -f "$candidate/NoritoBridge.xcframework/NoritoBridge.artifacts.json"
  printf '%s\\n' "$candidate" > "${NORITO_BRIDGE_CHECKER_MARKER:?}"
  exit "${NORITO_BRIDGE_CHECKER_EXIT:-0}"
fi
exec "${REAL_BASH:?}" "$@"
""",
    )


def _build_environment(
    tmp_path: Path,
    *,
    checker_exit: int = 0,
) -> tuple[Path, Path, dict[str, str], Path]:
    build_dir = tmp_path / "build"
    out_dir = tmp_path / "dist"
    tools_dir = tmp_path / "tools"
    checker_marker = tmp_path / "checker-candidate"
    tools_dir.mkdir()
    _write_live_artifact(out_dir)

    _write_static_library(build_dir, "aarch64-apple-ios", "device")
    _write_static_library(build_dir, "aarch64-apple-ios-sim", "sim-arm")
    _write_static_library(build_dir, "x86_64-apple-ios", "sim-x64")
    _write_static_library(build_dir, "aarch64-apple-darwin", "macos")
    _write_fake_tools(tools_dir)

    env = os.environ.copy()
    real_bash = shutil.which("bash", path=env.get("PATH"))
    assert real_bash is not None
    real_python3 = shutil.which("python3", path=env.get("PATH"))
    assert real_python3 is not None
    env.update(
        {
            "NORITO_BRIDGE_BUILD_DIR": str(build_dir),
            "NORITO_BRIDGE_OUT_DIR": str(out_dir),
            "NORITO_BRIDGE_SKIP_CARGO_BUILDS": "1",
            "NORITO_BRIDGE_CHECKER_EXIT": str(checker_exit),
            "NORITO_BRIDGE_CHECKER_MARKER": str(checker_marker),
            "PATH": f"{tools_dir}{os.pathsep}{env['PATH']}",
            "REAL_BASH": real_bash,
            "REAL_PYTHON3": real_python3,
        }
    )
    return build_dir, out_dir, env, checker_marker


def _run_builder(env: dict[str, str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            "bash",
            str(SCRIPT),
            "--bridge-version",
            "1.0.0",
            "--allow-dirty-source",
        ],
        cwd=ROOT,
        env=env,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def test_manual_xcframework_fallback_writes_required_info_plist(
    tmp_path: Path,
) -> None:
    _, out_dir, env, checker_marker = _build_environment(tmp_path)
    result = _run_builder(env)

    assert result.returncode == 0, result.stderr

    xcframework = out_dir / "NoritoBridge.xcframework"
    assert not (xcframework / "live-sentinel").exists()
    assert not (xcframework / "unexpected-partial").exists()
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

    public_manifest = out_dir / "NoritoBridge.artifacts.json"
    embedded_manifest = xcframework / "NoritoBridge.artifacts.json"
    assert public_manifest.is_symlink()
    assert (
        os.readlink(public_manifest)
        == "NoritoBridge.xcframework/NoritoBridge.artifacts.json"
    )
    assert public_manifest.resolve() == embedded_manifest.resolve()
    manifest = json.loads(public_manifest.read_text(encoding="utf-8"))
    assert manifest["native_bridge_abi_version"] == 21
    assert set(manifest["hashes"]) == set(SLICE_IDS)
    for identifier in SLICE_IDS:
        binary = xcframework / identifier / PUBLIC_STATIC_LIB_NAME
        assert manifest["hashes"][identifier] == hashlib.sha256(
            binary.read_bytes()
        ).hexdigest()

    checked_candidate = Path(checker_marker.read_text(encoding="utf-8").strip())
    assert checked_candidate.parent == out_dir
    assert checked_candidate.name.startswith(".NoritoBridge.publish.")
    assert not checked_candidate.exists()
    assert not list(out_dir.glob(".NoritoBridge.publish.*"))
    assert REPO_BUILD_LOCK.is_file()


def test_prepublication_checker_failure_preserves_live_pair(
    tmp_path: Path,
) -> None:
    _, out_dir, env, checker_marker = _build_environment(
        tmp_path,
        checker_exit=73,
    )
    live_framework = out_dir / "NoritoBridge.xcframework"
    live_manifest = out_dir / "NoritoBridge.artifacts.json"
    original_manifest = live_manifest.read_bytes()
    original_hashes = {
        identifier: hashlib.sha256(
            (live_framework / identifier / PUBLIC_STATIC_LIB_NAME).read_bytes()
        ).hexdigest()
        for identifier in SLICE_IDS
    }

    result = _run_builder(env)

    assert result.returncode == 73, result.stderr
    assert (live_framework / "live-sentinel").is_file()
    assert live_manifest.is_file()
    assert not live_manifest.is_symlink()
    assert live_manifest.read_bytes() == original_manifest
    assert not (live_framework / "NoritoBridge.artifacts.json").exists()
    for identifier, expected_hash in original_hashes.items():
        binary = live_framework / identifier / PUBLIC_STATIC_LIB_NAME
        assert hashlib.sha256(binary.read_bytes()).hexdigest() == expected_hash

    checked_candidate = Path(checker_marker.read_text(encoding="utf-8").strip())
    assert checked_candidate.parent == out_dir
    assert not checked_candidate.exists()
    assert not list(out_dir.glob(".NoritoBridge.publish.*"))
    assert REPO_BUILD_LOCK.is_file()


def test_termination_cleans_lock_and_candidate_without_touching_live_pair(
    tmp_path: Path,
) -> None:
    _, out_dir, env, _ = _build_environment(tmp_path)
    wait_marker = tmp_path / "xcodebuild-waiting"
    env["NORITO_BRIDGE_XCODE_WAIT_MARKER"] = str(wait_marker)
    live_framework = out_dir / "NoritoBridge.xcframework"
    live_manifest = out_dir / "NoritoBridge.artifacts.json"
    original_manifest = live_manifest.read_bytes()

    process = subprocess.Popen(
        [
            "bash",
            str(SCRIPT),
            "--bridge-version",
            "1.0.0",
            "--allow-dirty-source",
        ],
        cwd=ROOT,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
    )
    deadline = time.monotonic() + 10
    while not wait_marker.exists() and process.poll() is None:
        if time.monotonic() >= deadline:
            process.kill()
            raise AssertionError("builder did not reach the xcodebuild wait point")
        time.sleep(0.05)
    if not wait_marker.exists():
        _, stderr = process.communicate(timeout=5)
        raise AssertionError(f"builder exited before the wait point: {stderr}")

    os.killpg(process.pid, signal.SIGTERM)
    _, stderr = process.communicate(timeout=10)

    assert process.returncode == 143, stderr
    assert (live_framework / "live-sentinel").is_file()
    assert live_manifest.is_file()
    assert not live_manifest.is_symlink()
    assert live_manifest.read_bytes() == original_manifest
    assert not list(out_dir.glob(".NoritoBridge.publish.*"))
    assert REPO_BUILD_LOCK.is_file()


def test_builder_does_not_require_process_table_access(tmp_path: Path) -> None:
    _, out_dir, env, _ = _build_environment(tmp_path)
    tools_dir = Path(env["PATH"].split(os.pathsep, 1)[0])
    _write_executable(
        tools_dir / "ps",
        """#!/usr/bin/env bash
echo "process table access denied" >&2
exit 99
""",
    )

    result = _run_builder(env)
    assert result.returncode == 0, result.stderr
    assert (out_dir / "NoritoBridge.artifacts.json").is_symlink()
    assert REPO_BUILD_LOCK.is_file()
