"""Tests for the explicit Taira v2 24-hour soak launcher."""

from __future__ import annotations

import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "run_taira_v2_24h_soak.sh"
EXPECTED_TEST = (
    "taira_public_localnet::" "taira_profile_24h_packet_impairment_and_restart_soak"
)
HEAD_COMMIT = "1" * 40
HEAD_TREE = "2" * 40
CARGO_LOCK_SHA256 = "3" * 64
SOURCE_MANIFEST = "b" * 64
PINNED_ENV = {
    "IROHA_TEST_REQUIRE_NETWORK": "1",
    "IROHA_TAIRA_SIM_DURATION_SECS": "86400",
    "IROHA_TAIRA_SIM_SEED": "taira-public-sim",
    "IROHA_TAIRA_LOAD_TPS": "5",
    "IROHA_TAIRA_PACKET_LOSS_PERCENT": "10",
    "IROHA_TAIRA_CHURN_INTERVAL_SECS": "300",
    "IROHA_TAIRA_MAX_HEIGHT_SKEW": "2",
    "IROHA_TAIRA_MAX_HEIGHT_SKEW_GRACE_SECS": "30",
    "IROHA_TAIRA_MAX_TRANSIENT_HEIGHT_SKEW": "32",
    "IROHA_TAIRA_STALL_TIMEOUT_SECS": "300",
    "IROHA_TAIRA_MAX_VIEW_CHANGE_RATE": "0.2",
    "IROHA_TAIRA_MAX_LAGGED_CYCLE_RATIO": "0.35",
    "IROHA_TAIRA_MIN_COMMITTED_TPS_RATIO": "0.6",
    "IROHA_TAIRA_KEEP_LOCALNET": "1",
    "IROHA_TEST_SKIP_BUILD": "1",
    "IROHA_TEST_ALLOW_REENTRANT_BUILD": "0",
    "IROHA_TEST_BUILD_TIMEOUT_MS": "3600",
    "IROHA_TEST_BUILD_PROFILE": "release",
    "PROFILE": "release",
    "RUST_LOG": "info",
    "CARGO_NET_OFFLINE": "true",
}
_EXTERNAL_ROOTS: list[Path] = []


@pytest.fixture(autouse=True)
def _cleanup_external_roots() -> None:
    yield
    while _EXTERNAL_ROOTS:
        root = _EXTERNAL_ROOTS.pop()
        for path in root.rglob("*"):
            if not path.is_symlink() and path.is_dir():
                path.chmod(0o700)
        shutil.rmtree(root, ignore_errors=True)


def _install_source_bound_fake_localnet_binaries(
    program_target: Path,
) -> tuple[Path, str]:
    attestation = program_target / ".sumeragi-v2-prebuilt-binaries.tsv"
    if attestation.is_file():
        return program_target, hashlib.sha256(attestation.read_bytes()).hexdigest()
    binaries = {
        "irohad": program_target / "release" / "iroha3d",
        "irohad_message_control": (
            program_target / "message-control" / "release" / "iroha3d"
        ),
        "iroha": program_target / "release" / "iroha",
        "kagami": program_target / "release" / "kagami",
    }
    for label, binary in binaries.items():
        binary.parent.mkdir(parents=True, exist_ok=True)
        temporary = binary.with_name(
            f".{binary.name}.{os.getpid()}.{time.time_ns()}.tmp"
        )
        temporary.write_text(
            f"#!/bin/sh\nprintf '%s\\n' mocked-{label}\n",
            encoding="utf-8",
        )
        temporary.chmod(0o500)
        os.replace(temporary, binary)

    cargo_lock_sha256 = hashlib.sha256(
        (REPO_ROOT / "Cargo.lock").read_bytes()
    ).hexdigest()
    attestation_temporary = attestation.with_name(
        f".{attestation.name}.{os.getpid()}.{time.time_ns()}.tmp"
    )
    rows = [
        ("schema_version", "2"),
        ("source_manifest_sha256", SOURCE_MANIFEST),
        ("cargo_lock_sha256", cargo_lock_sha256),
        ("cargo_version_sha256", hashlib.sha256(b"cargo fixture\n").hexdigest()),
        ("rustc_version_sha256", hashlib.sha256(b"rustc fixture\n").hexdigest()),
        ("host_triple", "fixture-host"),
        ("target_triple", "fixture-host"),
        ("profile", "release"),
        ("bundle_dir", str(program_target)),
    ]
    for label, relative in (
        ("irohad", "release/iroha3d"),
        ("irohad_message_control", "message-control/release/iroha3d"),
        ("iroha", "release/iroha"),
        ("kagami", "release/kagami"),
    ):
        binary = binaries[label]
        rows.extend(
            (
                (f"{label}_relative_path", relative),
                (f"{label}_sha256", hashlib.sha256(binary.read_bytes()).hexdigest()),
                (f"{label}_size_bytes", str(binary.stat().st_size)),
                (f"{label}_mode_octal", "0500"),
            )
        )
    attestation_temporary.write_text(
        "".join(f"{key}\t{value}\n" for key, value in rows),
        encoding="utf-8",
    )
    attestation_temporary.chmod(0o400)
    os.replace(attestation_temporary, attestation)
    for directory in sorted(
        (path for path in program_target.rglob("*") if path.is_dir()),
        key=lambda path: len(path.parts),
        reverse=True,
    ):
        directory.chmod(0o500)
    program_target.chmod(0o500)
    return program_target, hashlib.sha256(attestation.read_bytes()).hexdigest()


def _stubbed_environment(
    tmp_path: Path,
    *,
    inventory_mode: str = "one",
    run_mode: str = "one",
    evidence_check_status: int = 0,
    program_target_name: str = "invocation.tairafixture",
) -> tuple[dict[str, str], Path]:
    external_root = Path(
        tempfile.mkdtemp(prefix="iroha-taira-v2-soak-test-", dir="/private/tmp")
    )
    _EXTERNAL_ROOTS.append(external_root)
    cargo_target_dir = external_root / "cargo-target"
    artifact_root = external_root / "artifacts"
    cargo_target_dir.mkdir(mode=0o700)
    artifact_root.mkdir(mode=0o700)
    program_target = (
        artifact_root
        / "sumeragi-v2-release"
        / SOURCE_MANIFEST
        / "programs"
        / program_target_name
    )
    program_target, manifest_sha256 = _install_source_bound_fake_localnet_binaries(
        program_target
    )
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    ps = bin_dir / "ps"
    ps.write_text(
        """#!/usr/bin/env bash
set -euo pipefail
case "$*" in
  "-axo pid,etime,command") printf '%s\n' '  PID ELAPSED COMMAND' ;;
  "-axo pid=,command=") ;;
  *) exit 64 ;;
esac
""",
        encoding="utf-8",
    )
    ps.chmod(0o755)
    marker_failure_harness = tmp_path / "fail-marker-parent-fsync.py"
    marker_failure_harness.write_text(
        """import errno
import importlib.util
import os
from pathlib import Path
import sys

publisher = Path(os.environ["TAIRA_EXPECTED_MARKER_PUBLISHER"])
spec = importlib.util.spec_from_file_location("release_marker_publisher", publisher)
assert spec is not None and spec.loader is not None
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
real_fsync = module.os.fsync
fsync_calls = 0


def fail_completion_parent_fsync(descriptor: int) -> None:
    global fsync_calls
    fsync_calls += 1
    if fsync_calls == 4:
        raise OSError(errno.EIO, "mocked completion-parent fsync failure")
    real_fsync(descriptor)


module.os.fsync = fail_completion_parent_fsync
raise SystemExit(module.main(sys.argv[1:]))
""",
        encoding="utf-8",
    )
    checker_capture = tmp_path / "evidence-checker-invocations.txt"
    python = bin_dir / "python3"
    python.write_text(
        f"""#!/bin/sh
if [ "${{1-}}" = "-I" ] \
  && [ "${{2-}}" = "-S" ] \
  && [ "${{3-}}" = "$TAIRA_EXPECTED_MARKER_PUBLISHER" ]; then
  if [ "${{TAIRA_FAIL_MARKER_PUBLISH:-0}}" = 1 ]; then
    shift 3
    exec "$TAIRA_REAL_PYTHON3" "$TAIRA_MARKER_FAILURE_HARNESS" "$@"
  fi
  exec "$TAIRA_REAL_PYTHON3" "$@"
fi
case "$1" in
  *compute_workspace_source_manifest.py) printf '%s\n' '{SOURCE_MANIFEST}' ;;
  *check_taira_v2_soak_evidence.py)
    printf '%s\n' "$*" >>"$TAIRA_EVIDENCE_CHECK_CAPTURE"
    exit {evidence_check_status}
    ;;
  *) exec "$TAIRA_REAL_PYTHON3" "$@" ;;
esac
""",
        encoding="utf-8",
    )
    python.chmod(0o755)
    capture = tmp_path / "cargo-invocations.txt"
    cargo = bin_dir / "cargo"
    cargo.write_text(
        f"""#!/usr/bin/env bash
set -euo pipefail
{{
  printf 'args=%s\\n' "$*"
  printf '%s\\n' \\
    "IROHA_TEST_REQUIRE_NETWORK=${{IROHA_TEST_REQUIRE_NETWORK-<unset>}}" \\
    "IROHA_TAIRA_SIM_DURATION_SECS=${{IROHA_TAIRA_SIM_DURATION_SECS-<unset>}}" \\
    "IROHA_TAIRA_SIM_SEED=${{IROHA_TAIRA_SIM_SEED-<unset>}}" \\
    "IROHA_TAIRA_LOAD_TPS=${{IROHA_TAIRA_LOAD_TPS-<unset>}}" \\
    "IROHA_TAIRA_PACKET_LOSS_PERCENT=${{IROHA_TAIRA_PACKET_LOSS_PERCENT-<unset>}}" \\
    "IROHA_TAIRA_CHURN_INTERVAL_SECS=${{IROHA_TAIRA_CHURN_INTERVAL_SECS-<unset>}}" \\
    "IROHA_TAIRA_MAX_HEIGHT_SKEW=${{IROHA_TAIRA_MAX_HEIGHT_SKEW-<unset>}}" \\
    "IROHA_TAIRA_MAX_HEIGHT_SKEW_GRACE_SECS=${{IROHA_TAIRA_MAX_HEIGHT_SKEW_GRACE_SECS-<unset>}}" \\
    "IROHA_TAIRA_MAX_TRANSIENT_HEIGHT_SKEW=${{IROHA_TAIRA_MAX_TRANSIENT_HEIGHT_SKEW-<unset>}}" \\
    "IROHA_TAIRA_STALL_TIMEOUT_SECS=${{IROHA_TAIRA_STALL_TIMEOUT_SECS-<unset>}}" \\
    "IROHA_TAIRA_MAX_VIEW_CHANGE_RATE=${{IROHA_TAIRA_MAX_VIEW_CHANGE_RATE-<unset>}}" \\
    "IROHA_TAIRA_MAX_LAGGED_CYCLE_RATIO=${{IROHA_TAIRA_MAX_LAGGED_CYCLE_RATIO-<unset>}}" \\
    "IROHA_TAIRA_MIN_COMMITTED_TPS_RATIO=${{IROHA_TAIRA_MIN_COMMITTED_TPS_RATIO-<unset>}}" \\
    "IROHA_TAIRA_KEEP_LOCALNET=${{IROHA_TAIRA_KEEP_LOCALNET-<unset>}}" \\
    "IROHA_TEST_SKIP_BUILD=${{IROHA_TEST_SKIP_BUILD-<unset>}}" \\
    "IROHA_TEST_ALLOW_REENTRANT_BUILD=${{IROHA_TEST_ALLOW_REENTRANT_BUILD-<unset>}}" \\
    "IROHA_TEST_BUILD_TIMEOUT_MS=${{IROHA_TEST_BUILD_TIMEOUT_MS-<unset>}}" \\
    "IROHA_TEST_BUILD_PROFILE=${{IROHA_TEST_BUILD_PROFILE-<unset>}}" \\
    "PROFILE=${{PROFILE-<unset>}}" \\
    "RUST_LOG=${{RUST_LOG-<unset>}}" \\
    "CARGO_NET_OFFLINE=${{CARGO_NET_OFFLINE-<unset>}}" \\
    "IROHA_RELEASE_SOURCE_MANIFEST_SHA256=${{IROHA_RELEASE_SOURCE_MANIFEST_SHA256-<unset>}}" \\
    "IROHA_TAIRA_EVIDENCE_PATH=${{IROHA_TAIRA_EVIDENCE_PATH-<unset>}}" \\
    "IROHA_TEST_TARGET_DIR=${{IROHA_TEST_TARGET_DIR-<unset>}}" \\
    "CARGO_TARGET_DIR=${{CARGO_TARGET_DIR-<unset>}}" \\
    "TEST_NETWORK_BIN_IROHAD=${{TEST_NETWORK_BIN_IROHAD-<unset>}}" \\
    "KAGAMI_BIN=${{KAGAMI_BIN-<unset>}}" \\
    "TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL=${{TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL-<unset>}}" \\
    "TEST_NETWORK_BIN_IROHA=${{TEST_NETWORK_BIN_IROHA-<unset>}}" \\
    "TEST_NETWORK_IROHAD_FEATURES=${{TEST_NETWORK_IROHAD_FEATURES-<unset>}}" \\
    "CARGO_BIN_EXE_iroha=${{CARGO_BIN_EXE_iroha-<unset>}}"
  printf '%s\\n' '--'
}} >>"$TAIRA_SOAK_CAPTURE"

case " $* " in
  *" --list "*)
    case "${{TAIRA_FAKE_INVENTORY_MODE:-one}}" in
      one) printf '%s\\n' '{EXPECTED_TEST}: test' ;;
      zero) ;;
      duplicate)
        printf '%s\\n' '{EXPECTED_TEST}: test' '{EXPECTED_TEST}: test'
        ;;
      *) exit 64 ;;
    esac
    ;;
  *)
    # Mirror required_release_evidence_path in taira_public_localnet: the
    # release path must be absolute and its final extension must be `json`.
    case "$IROHA_TAIRA_EVIDENCE_PATH" in
      /*.json) ;;
      *)
        printf '%s\n' 'IROHA_TAIRA_EVIDENCE_PATH must name an absolute JSON file' >&2
        exit 66
        ;;
    esac
    case "${{TAIRA_FAKE_RUN_MODE:-one}}" in
      one)
        mkdir -p "$(dirname "$IROHA_TAIRA_EVIDENCE_PATH")"
        printf '%s\n' '{{}}' >"$IROHA_TAIRA_EVIDENCE_PATH"
        printf '%s\\n' \\
          'running 1 test' \\
          'test {EXPECTED_TEST} ... ok' \\
          '' \\
          'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 42 filtered out; finished in 0.01s'
        ;;
      tamper-bundle)
        mkdir -p "$(dirname "$IROHA_TAIRA_EVIDENCE_PATH")"
        printf '%s\n' '{{}}' >"$IROHA_TAIRA_EVIDENCE_PATH"
        printf '%s\\n' \\
          'running 1 test' \\
          'test {EXPECTED_TEST} ... ok' \\
          '' \\
          'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 42 filtered out; finished in 0.01s'
        chmod 0700 "$TAIRA_TAMPER_BINARY"
        printf '%s\n' 'tampered after process admission' >"$TAIRA_TAMPER_BINARY"
        chmod 0500 "$TAIRA_TAMPER_BINARY"
        ;;
      marker-temp-symlink)
        mkdir -p "$(dirname "$IROHA_TAIRA_EVIDENCE_PATH")"
        printf '%s\n' '{{}}' >"$IROHA_TAIRA_EVIDENCE_PATH"
        ln -s "$TAIRA_ESCAPE_TARGET" \
          "$(dirname "$IROHA_TAIRA_EVIDENCE_PATH")/.COMPLETED.tsv.publish.tmp"
        printf '%s\\n' \\
          'running 1 test' \\
          'test {EXPECTED_TEST} ... ok' \\
          '' \\
          'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 42 filtered out; finished in 0.01s'
        ;;
      zero)
        printf '%s\\n' \\
          'running 0 tests' \\
          '' \\
          'test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 43 filtered out; finished in 0.00s'
        ;;
      *) exit 65 ;;
    esac
    ;;
esac
""",
        encoding="utf-8",
    )
    cargo.chmod(0o755)
    env = os.environ.copy()
    env["PATH"] = f"{bin_dir}:{env['PATH']}"
    env["IROHA_RELEASE_CARGO_BIN"] = str(cargo)
    env["TAIRA_SOAK_CAPTURE"] = str(capture)
    env["TAIRA_EVIDENCE_CHECK_CAPTURE"] = str(checker_capture)
    env["TAIRA_FAKE_INVENTORY_MODE"] = inventory_mode
    env["TAIRA_FAKE_RUN_MODE"] = run_mode
    env["IROHA_RELEASE_HEAD_COMMIT"] = HEAD_COMMIT
    env["IROHA_RELEASE_HEAD_TREE"] = HEAD_TREE
    env["IROHA_RELEASE_CARGO_LOCK_SHA256"] = CARGO_LOCK_SHA256
    env["CARGO_TARGET_DIR"] = str(cargo_target_dir)
    env["IROHA_RELEASE_ARTIFACT_ROOT"] = str(artifact_root)
    env["IROHA_RELEASE_CANCEL_REQUEST_PATH"] = str(
        external_root / "cancel-request.json"
    )
    env["IROHA_TEST_TARGET_DIR"] = str(program_target)
    env["IROHA_RELEASE_PREBUILT_MANIFEST_SHA256"] = manifest_sha256
    env["TAIRA_REAL_PYTHON3"] = sys.executable
    env["TAIRA_EXPECTED_MARKER_PUBLISHER"] = str(
        REPO_ROOT / "scripts" / "publish_release_marker.py"
    )
    env["TAIRA_MARKER_FAILURE_HARNESS"] = str(marker_failure_harness)
    return env, capture


def _run_launcher(env: dict[str, str], *args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [str(SCRIPT), *args],
        cwd=REPO_ROOT,
        env=env,
        check=False,
        capture_output=True,
        text=True,
    )


def test_launcher_pins_complete_profile_and_runs_exactly_one_test(
    tmp_path: Path,
) -> None:
    env, capture = _stubbed_environment(tmp_path)
    env.update({name: "inherited-malicious-override" for name in PINNED_ENV})
    env["TEST_NETWORK_BIN_IROHAD"] = "/tmp/malicious-iroha3d"
    env["KAGAMI_BIN"] = "/tmp/malicious-kagami"
    env["TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL"] = "/tmp/malicious-controlled-iroha3d"
    env["TEST_NETWORK_BIN_IROHA"] = "/tmp/malicious-iroha"
    env["TEST_NETWORK_IROHAD_FEATURES"] = "malicious-feature"
    env["CARGO_BIN_EXE_iroha"] = "/tmp/malicious-cargo-iroha"
    env["IROHA_RELEASE_SOURCE_MANIFEST_SHA256"] = "0" * 64
    env["IROHA_TAIRA_EVIDENCE_PATH"] = "/tmp/malicious-evidence.json"
    completion_pointer = tmp_path / "taira-completion-path"
    env["IROHA_TAIRA_COMPLETION_PATH_FILE"] = str(completion_pointer)

    mismatch = _run_launcher(env)

    assert mismatch.returncode == 1
    assert "does not match the parent release invocation" in mismatch.stderr
    assert not capture.exists()

    env["IROHA_RELEASE_SOURCE_MANIFEST_SHA256"] = SOURCE_MANIFEST
    result = _run_launcher(env)

    assert result.returncode == 0, result.stderr
    captured = capture.read_text(encoding="utf-8")
    calls = [line for line in captured.splitlines() if line.startswith("args=")]
    assert len(calls) == 2
    assert all(
        "test -j1 --locked --offline --release -p integration_tests "
        "--test consensus_and_da"
        in call
        for call in calls
    )
    assert "-- --list --ignored" in calls[0]
    assert EXPECTED_TEST in calls[1]
    assert "-- --exact --ignored --nocapture --test-threads=1" in calls[1]
    captured_lines = captured.splitlines()
    for name, value in PINNED_ENV.items():
        assert captured_lines.count(f"{name}={value}") == 2
        assert f"{name}=inherited-malicious-override" not in captured_lines
    artifact_root = Path(env["IROHA_RELEASE_ARTIFACT_ROOT"])
    cargo_target_dir = Path(env["CARGO_TARGET_DIR"])
    source_root = artifact_root / "sumeragi-v2-release" / SOURCE_MANIFEST
    evidence_root = source_root / "evidence" / "taira-v2-24h"
    assert not (evidence_root / ".taira_v2_24h_soak.lock").exists()
    assert (
        captured.count(
            f"IROHA_RELEASE_SOURCE_MANIFEST_SHA256={SOURCE_MANIFEST}\n"
        )
        == 2
    )
    program_target = Path(env["IROHA_TEST_TARGET_DIR"])
    assert captured.count(f"IROHA_TEST_TARGET_DIR={program_target}\n") == 2
    assert captured.count(f"CARGO_TARGET_DIR={cargo_target_dir}\n") == 2
    evidence_values = {
        line.split("=", 1)[1]
        for line in captured_lines
        if line.startswith("IROHA_TAIRA_EVIDENCE_PATH=")
    }
    assert len(evidence_values) == 1
    partial_evidence = Path(evidence_values.pop())
    assert partial_evidence.parent.parent == evidence_root
    assert partial_evidence.name == ".taira_v2_24h_soak.partial.json"
    assert partial_evidence.is_absolute()
    assert partial_evidence.suffix == ".json"
    assert not partial_evidence.exists()
    durable_evidence = partial_evidence.with_name("taira_v2_24h_soak.json")
    completion = partial_evidence.with_name("COMPLETED.tsv")
    run_log = partial_evidence.with_name("taira-v2-24h.log")
    assert durable_evidence.is_file()
    assert run_log.is_file()
    assert f"test {EXPECTED_TEST} ... ok" in run_log.read_text(encoding="utf-8")
    assert completion.is_file()
    completion_fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    assert completion_fields["log_sha256"] == hashlib.sha256(
        run_log.read_bytes()
    ).hexdigest()
    assert (
        completion_fields["prebuilt_manifest_sha256"]
        == env["IROHA_RELEASE_PREBUILT_MANIFEST_SHA256"]
    )
    assert completion_pointer.read_text(encoding="utf-8").strip() == str(completion)
    checker_calls = Path(env["TAIRA_EVIDENCE_CHECK_CAPTURE"]).read_text(
        encoding="utf-8"
    ).splitlines()
    assert len(checker_calls) == 1
    checker_arguments = checker_calls[0].split()
    assert checker_arguments.count("--cargo-target-dir") == 1
    target_index = checker_arguments.index("--cargo-target-dir")
    assert checker_arguments[target_index + 1] == str(cargo_target_dir)
    assert (
        captured.count(
            f"TEST_NETWORK_BIN_IROHAD={program_target / 'release' / 'iroha3d'}\n"
        )
        == 2
    )
    assert (
        captured.count(f"KAGAMI_BIN={program_target / 'release' / 'kagami'}\n")
        == 2
    )
    assert (
        captured.count(
            "TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL="
            f"{program_target / 'message-control' / 'release' / 'iroha3d'}\n"
        )
        == 2
    )
    assert (
        captured.count(
            f"TEST_NETWORK_BIN_IROHA={program_target / 'release' / 'iroha'}\n"
        )
        == 2
    )
    assert captured.count("TEST_NETWORK_IROHAD_FEATURES=<unset>\n") == 2
    assert captured.count("CARGO_BIN_EXE_iroha=<unset>\n") == 2
    assert "passed with exactly one test" in result.stderr


def test_launcher_rejects_zero_test_inventory(tmp_path: Path) -> None:
    env, capture = _stubbed_environment(tmp_path, inventory_mode="zero")

    result = _run_launcher(env)

    assert result.returncode == 1
    assert "expected exactly one ignored Taira soak" in result.stderr
    captured = capture.read_text(encoding="utf-8")
    assert captured.count("args=") == 1


def test_launcher_rejects_zero_test_execution_output(tmp_path: Path) -> None:
    env, capture = _stubbed_environment(tmp_path, run_mode="zero")

    result = _run_launcher(env)

    assert result.returncode == 1
    assert "expected exactly one Taira soak test to run and pass" in result.stderr
    assert "running 0 tests" in result.stdout
    captured = capture.read_text(encoding="utf-8")
    assert captured.count("args=") == 2


def test_launcher_rejects_bundle_tampering_before_completion(
    tmp_path: Path,
) -> None:
    invocation_suffix = hashlib.sha256(str(tmp_path).encode()).hexdigest()[:16]
    env, _capture = _stubbed_environment(
        tmp_path,
        run_mode="tamper-bundle",
        program_target_name=f"invocation.T{invocation_suffix}",
    )
    program_target = Path(env["IROHA_TEST_TARGET_DIR"])
    binary = program_target / "release" / "iroha3d"
    original = binary.read_bytes()
    env["TAIRA_TAMPER_BINARY"] = str(binary)
    completion_pointer = tmp_path / "taira-completion-path"
    env["IROHA_TAIRA_COMPLETION_PATH_FILE"] = str(completion_pointer)
    try:
        result = _run_launcher(env)
    finally:
        binary.chmod(0o700)
        binary.write_bytes(original)
        binary.chmod(0o500)

    assert result.returncode == 1
    assert "binary bundle changed before Taira completion" in result.stderr
    assert not completion_pointer.exists()


def test_launcher_rejects_symlinked_marker_temp_without_completion(
    tmp_path: Path,
) -> None:
    env, capture = _stubbed_environment(
        tmp_path,
        run_mode="marker-temp-symlink",
    )
    escape = tmp_path / "marker-temp-escape"
    escape.write_text("must remain unchanged\n", encoding="utf-8")
    env["TAIRA_ESCAPE_TARGET"] = str(escape)
    completion_pointer = tmp_path / "taira-completion-path"
    env["IROHA_TAIRA_COMPLETION_PATH_FILE"] = str(completion_pointer)

    result = _run_launcher(env)

    assert result.returncode == 1
    assert "completion marker temporary already exists as symlink" in result.stderr
    partial_values = {
        Path(line.split("=", 1)[1])
        for line in capture.read_text(encoding="utf-8").splitlines()
        if line.startswith("IROHA_TAIRA_EVIDENCE_PATH=")
    }
    assert len(partial_values) == 1
    invocation = partial_values.pop().parent
    assert (invocation / ".COMPLETED.tsv.publish.tmp").is_symlink()
    assert escape.read_text(encoding="utf-8") == "must remain unchanged\n"
    assert not (invocation / "COMPLETED.tsv").exists()
    assert not (invocation / "taira_v2_24h_soak.json").exists()
    assert not completion_pointer.exists()


def test_launcher_marker_durability_failure_is_not_terminal(
    tmp_path: Path,
) -> None:
    env, capture = _stubbed_environment(tmp_path)
    env["TAIRA_FAIL_MARKER_PUBLISH"] = "1"
    completion_pointer = tmp_path / "taira-completion-path"
    env["IROHA_TAIRA_COMPLETION_PATH_FILE"] = str(completion_pointer)

    result = _run_launcher(env)

    assert result.returncode == 1
    assert "mocked completion-parent fsync failure" in result.stderr
    partial_values = {
        Path(line.split("=", 1)[1])
        for line in capture.read_text(encoding="utf-8").splitlines()
        if line.startswith("IROHA_TAIRA_EVIDENCE_PATH=")
    }
    assert len(partial_values) == 1
    invocation = partial_values.pop().parent
    assert not (invocation / "COMPLETED.tsv").exists()
    assert not (invocation / "taira_v2_24h_soak.json").exists()
    assert not completion_pointer.exists()


def test_launcher_rejects_profile_override_arguments_before_cargo(
    tmp_path: Path,
) -> None:
    env, capture = _stubbed_environment(tmp_path)

    result = _run_launcher(env, "--duration-secs", "30")

    assert result.returncode == 2
    assert "profile overrides are not supported" in result.stderr
    assert not capture.exists()


def test_launcher_rejects_a_concurrent_source_bound_soak(tmp_path: Path) -> None:
    env, capture = _stubbed_environment(tmp_path)
    source_root = (
        Path(env["IROHA_RELEASE_ARTIFACT_ROOT"])
        / "sumeragi-v2-release"
        / SOURCE_MANIFEST
    )
    lock_path = source_root / "evidence" / "taira-v2-24h" / ".taira_v2_24h_soak.lock"
    lock_path.mkdir(parents=True, exist_ok=False)
    try:
        result = _run_launcher(env)
    finally:
        lock_path.rmdir()

    assert result.returncode == 1
    assert "refusing shared release evidence" in result.stderr
    assert not capture.exists()


def test_launcher_does_not_promote_provisional_evidence_when_validation_fails(
    tmp_path: Path,
) -> None:
    env, capture = _stubbed_environment(tmp_path, evidence_check_status=71)
    completion_pointer = tmp_path / "taira-completion-path"
    env["IROHA_TAIRA_COMPLETION_PATH_FILE"] = str(completion_pointer)

    result = _run_launcher(env)

    assert result.returncode == 71
    captured = capture.read_text(encoding="utf-8")
    partial_values = {
        Path(line.split("=", 1)[1])
        for line in captured.splitlines()
        if line.startswith("IROHA_TAIRA_EVIDENCE_PATH=")
    }
    assert len(partial_values) == 1
    partial = partial_values.pop()
    assert not partial.exists()
    assert not partial.with_name("taira_v2_24h_soak.json").exists()
    assert not partial.with_name("COMPLETED.tsv").exists()
    assert partial.with_name("taira-v2-24h.log").is_file()
    assert not completion_pointer.exists()
