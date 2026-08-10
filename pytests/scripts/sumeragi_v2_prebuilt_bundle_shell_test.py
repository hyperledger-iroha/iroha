"""Shell integration tests for Sumeragi v2 prebuilt-bundle propagation."""

from __future__ import annotations

import os
from pathlib import Path
import shutil
import subprocess

import pytest


ROOT_DIR = Path(__file__).resolve().parents[2]
HELPER = ROOT_DIR / "scripts" / "sumeragi_v2_prebuilt_bundle.py"
SHELL_HELPER = ROOT_DIR / "scripts" / "sumeragi_v2_prebuilt_bundle.sh"
PROCESS_POLICY = ROOT_DIR / "scripts" / "sumeragi_v2_release_process_policy.sh"
SOURCE_MANIFEST = "c" * 64


def _shell_fixture(tmp_path: Path) -> tuple[Path, dict[str, str], Path]:
    repo = tmp_path.resolve() / "repo"
    scripts = repo / "scripts"
    scripts.mkdir(parents=True)
    shutil.copyfile(HELPER, scripts / HELPER.name)
    (repo / "Cargo.lock").write_bytes(b"shell-fixture-lock\n")

    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    cargo = fake_bin / "cargo"
    cargo.write_text(
        """#!/usr/bin/env bash
set -euo pipefail
[[ "$#" == 1 && "$1" == "--version" ]]
printf '%s\n' 'cargo 1.99.0 (shell-fixture)'
exit "${BUNDLE_TEST_CARGO_VERSION_STATUS:-0}"
""",
        encoding="utf-8",
    )
    cargo.chmod(0o755)
    rustc = fake_bin / "rustc"
    rustc.write_text(
        """#!/usr/bin/env bash
set -euo pipefail
[[ "$#" == 1 && "$1" == "-vV" ]]
printf '%s\n' \
  'rustc 1.99.0 (shell-fixture)' \
  'binary: rustc' \
  'commit-hash: fixture' \
  'commit-date: 2099-01-01' \
  'host: fixture-shell-host' \
  'release: 1.99.0' \
  'LLVM version: 99.0.0'
exit "${BUNDLE_TEST_RUSTC_VERSION_STATUS:-0}"
""",
        encoding="utf-8",
    )
    rustc.chmod(0o755)
    build_log = tmp_path / "builds.log"
    cargo_target = tmp_path / "cargo-target"
    artifact_root = tmp_path / "artifacts"
    cargo_target.mkdir(mode=0o700)
    artifact_root.mkdir(mode=0o700)
    env = os.environ.copy()
    env["PATH"] = f"{fake_bin}{os.pathsep}{env['PATH']}"
    env["BUNDLE_TEST_BUILD_LOG"] = str(build_log)
    env["CARGO_TARGET_DIR"] = str(cargo_target)
    env["IROHA_RELEASE_ARTIFACT_ROOT"] = str(artifact_root)
    env["IROHA_RELEASE_CANCEL_REQUEST_PATH"] = str(tmp_path / "cancel-request.json")
    return repo, env, build_log


def _run_harness(repo: Path, env: dict[str, str], body: str) -> subprocess.CompletedProcess[str]:
    script = f"""set -euo pipefail
source {shlex_quote(str(PROCESS_POLICY))}
require_external_private_directory() {{ :; }}
source {shlex_quote(str(SHELL_HELPER))}
wait_for_external_cargo() {{ :; }}
run_cargo() {{
  if [[ "${{1-}}" == --version ]]; then
    command cargo --version
    return
  fi
  [[ "${{1-}}" == build ]]
  mkdir -p -- "$CARGO_TARGET_DIR/release"
  case " $* " in
    *" -p irohad "*" --features test-network-message-control "*)
      output="$CARGO_TARGET_DIR/release/iroha3d"
      ;;
    *" -p irohad "*)
      output="$CARGO_TARGET_DIR/release/iroha3d"
      ;;
    *" -p iroha_cli "*)
      output="$CARGO_TARGET_DIR/release/iroha"
      ;;
    *" -p iroha_kagami "*)
      output="$CARGO_TARGET_DIR/release/kagami"
      ;;
    *)
      printf 'unexpected build: %s\\n' "$*" >&2
      return 65
      ;;
  esac
  printf '#!/bin/sh\\n# %s\\n' "$*" >"$output"
  chmod 0755 "$output"
  printf '%s\\n' "$*" >>"$BUNDLE_TEST_BUILD_LOG"
  if [[ -n "${{BUNDLE_TEST_FAIL_BUILD_MATCH:-}}" \
    && " $* " == *"${{BUNDLE_TEST_FAIL_BUILD_MATCH}}"* ]]; then
    return 71
  fi
}}
{body}
"""
    return subprocess.run(
        ["bash", "-c", script],
        cwd=repo,
        env=env,
        check=False,
        capture_output=True,
        text=True,
    )


def shlex_quote(value: str) -> str:
    return "'" + value.replace("'", "'\"'\"'") + "'"


def test_standalone_creation_ignores_unanchored_target_and_exports_exact_bundle(
    tmp_path: Path,
) -> None:
    repo, env, build_log = _shell_fixture(tmp_path)
    inherited_target = tmp_path / "environment-controlled-target"
    result = _run_harness(
        repo,
        env,
        f"""
export IROHA_TEST_TARGET_DIR={shlex_quote(str(inherited_target))}
unset IROHA_RELEASE_PREBUILT_MANIFEST_SHA256
sumeragi_v2_ensure_source_bound_localnet_binaries \
  {shlex_quote(str(repo))} {SOURCE_MANIFEST}
sumeragi_v2_export_source_bound_localnet_binaries \
  {shlex_quote(str(repo))} {SOURCE_MANIFEST}
printf '%s\\n' \
  "$IROHA_TEST_TARGET_DIR" \
  "$IROHA_RELEASE_PREBUILT_MANIFEST_SHA256" \
  "$TEST_NETWORK_BIN_IROHAD" \
  "$TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL" \
  "$TEST_NETWORK_BIN_IROHA" \
  "$KAGAMI_BIN"
""",
    )

    assert result.returncode == 0, result.stderr
    lines = result.stdout.splitlines()
    assert len(lines) == 6
    bundle = Path(lines[0])
    assert bundle != inherited_target
    assert bundle.parent == (
        Path(env["IROHA_RELEASE_ARTIFACT_ROOT"])
        / "sumeragi-v2-release"
        / SOURCE_MANIFEST
        / "programs"
    )
    assert bundle.name.startswith("invocation.")
    assert len(lines[1]) == 64
    assert lines[2:] == [
        str(bundle / "release" / "iroha3d"),
        str(bundle / "message-control" / "release" / "iroha3d"),
        str(bundle / "release" / "iroha"),
        str(bundle / "release" / "kagami"),
    ]
    assert len(build_log.read_text(encoding="utf-8").splitlines()) == 4


def test_inherited_anchor_is_reused_but_invalid_anchor_never_rebuilds(
    tmp_path: Path,
) -> None:
    repo, env, build_log = _shell_fixture(tmp_path)
    result = _run_harness(
        repo,
        env,
        f"""
sumeragi_v2_ensure_source_bound_localnet_binaries \
  {shlex_quote(str(repo))} {SOURCE_MANIFEST}
first_bundle="$IROHA_TEST_TARGET_DIR"
first_anchor="$IROHA_RELEASE_PREBUILT_MANIFEST_SHA256"
sumeragi_v2_ensure_source_bound_localnet_binaries \
  {shlex_quote(str(repo))} {SOURCE_MANIFEST}
[[ "$IROHA_TEST_TARGET_DIR" == "$first_bundle" ]]
[[ "$IROHA_RELEASE_PREBUILT_MANIFEST_SHA256" == "$first_anchor" ]]
export IROHA_RELEASE_PREBUILT_MANIFEST_SHA256={'0' * 64}
if sumeragi_v2_ensure_source_bound_localnet_binaries \
  {shlex_quote(str(repo))} {SOURCE_MANIFEST}; then
  exit 66
fi
printf '%s\\n' "$first_bundle" "$first_anchor"
""",
    )

    assert result.returncode == 0, result.stderr
    assert len(result.stdout.splitlines()) == 2
    assert len(build_log.read_text(encoding="utf-8").splitlines()) == 4
    assert "inherited release prebuilt manifest" in result.stderr


def test_clearing_anchor_forces_a_distinct_fresh_invocation(
    tmp_path: Path,
) -> None:
    repo, env, build_log = _shell_fixture(tmp_path)
    result = _run_harness(
        repo,
        env,
        f"""
sumeragi_v2_ensure_source_bound_localnet_binaries \
  {shlex_quote(str(repo))} {SOURCE_MANIFEST}
first_bundle="$IROHA_TEST_TARGET_DIR"
first_anchor="$IROHA_RELEASE_PREBUILT_MANIFEST_SHA256"
unset IROHA_RELEASE_PREBUILT_MANIFEST_SHA256
sumeragi_v2_ensure_source_bound_localnet_binaries \
  {shlex_quote(str(repo))} {SOURCE_MANIFEST}
[[ "$IROHA_TEST_TARGET_DIR" != "$first_bundle" ]]
[[ "$IROHA_RELEASE_PREBUILT_MANIFEST_SHA256" != "$first_anchor" ]]
printf '%s\\n' "$first_bundle" "$IROHA_TEST_TARGET_DIR"
""",
    )

    assert result.returncode == 0, result.stderr
    first, second = result.stdout.splitlines()
    assert first != second
    assert len(build_log.read_text(encoding="utf-8").splitlines()) == 8


def test_creation_ignores_repository_target_symlink_authority(
    tmp_path: Path,
) -> None:
    repo, env, build_log = _shell_fixture(tmp_path)
    workspace_target = tmp_path / "isolated-workspace-target"
    workspace_target.mkdir(mode=0o700)
    (repo / "target").symlink_to(workspace_target, target_is_directory=True)

    result = _run_harness(
        repo,
        env,
        f"""
sumeragi_v2_ensure_source_bound_localnet_binaries \
  {shlex_quote(str(repo))} {SOURCE_MANIFEST}
printf '%s\\n' "$IROHA_TEST_TARGET_DIR"
""",
    )

    assert result.returncode == 0, result.stderr
    bundle = Path(result.stdout.strip())
    assert bundle.parent == (
        Path(env["IROHA_RELEASE_ARTIFACT_ROOT"])
        / "sumeragi-v2-release"
        / SOURCE_MANIFEST
        / "programs"
    )
    assert (repo / "target").is_symlink()
    assert not list(workspace_target.iterdir())
    assert len(build_log.read_text(encoding="utf-8").splitlines()) == 4


def test_failed_build_cannot_fall_through_to_bundle_publication(
    tmp_path: Path,
) -> None:
    repo, env, build_log = _shell_fixture(tmp_path)
    env["BUNDLE_TEST_FAIL_BUILD_MATCH"] = "-p iroha_cli"

    result = _run_harness(
        repo,
        env,
        f"""
sumeragi_v2_ensure_source_bound_localnet_binaries \
  {shlex_quote(str(repo))} {SOURCE_MANIFEST}
""",
    )

    assert result.returncode != 0
    assert len(build_log.read_text(encoding="utf-8").splitlines()) == 2
    programs = (
        Path(env["IROHA_RELEASE_ARTIFACT_ROOT"])
        / "sumeragi-v2-release"
        / SOURCE_MANIFEST
        / "programs"
    )
    assert not programs.exists() or not list(programs.glob("invocation.*"))

    nested_repo, nested_env, nested_build_log = _shell_fixture(tmp_path / "nested")
    nested_artifacts = Path(nested_env["CARGO_TARGET_DIR"]) / "artifacts"
    nested_artifacts.mkdir()
    nested_env["IROHA_RELEASE_ARTIFACT_ROOT"] = str(nested_artifacts)
    nested = _run_harness(
        nested_repo,
        nested_env,
        f"sumeragi_v2_ensure_source_bound_localnet_binaries "
        f"{shlex_quote(str(nested_repo))} {SOURCE_MANIFEST}",
    )
    assert nested.returncode == 2
    assert not nested_build_log.exists()


@pytest.mark.parametrize(
    "status_variable",
    ("BUNDLE_TEST_CARGO_VERSION_STATUS", "BUNDLE_TEST_RUSTC_VERSION_STATUS"),
)
def test_failed_tool_probe_cannot_publish_plausible_stdout(
    tmp_path: Path,
    status_variable: str,
) -> None:
    repo, env, build_log = _shell_fixture(tmp_path)
    env[status_variable] = "72"

    result = _run_harness(
        repo,
        env,
        f"""
sumeragi_v2_ensure_source_bound_localnet_binaries \
  {shlex_quote(str(repo))} {SOURCE_MANIFEST}
""",
    )

    assert result.returncode != 0
    assert len(build_log.read_text(encoding="utf-8").splitlines()) == 4
    programs = (
        Path(env["IROHA_RELEASE_ARTIFACT_ROOT"])
        / "sumeragi-v2-release"
        / SOURCE_MANIFEST
        / "programs"
    )
    assert not programs.exists() or not list(programs.glob("invocation.*"))
