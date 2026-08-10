"""Mocked-Cargo contract tests for the Sumeragi v2 deterministic seed runner.

These tests exercise the shell launcher's accounting and failure handling; they
are not evidence that any real validator process started or made progress.
"""

from __future__ import annotations

import atexit
import csv
import hashlib
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path


ROOT_DIR = Path(__file__).resolve().parents[2]
SCRIPT = ROOT_DIR / "scripts" / "run_sumeragi_v2_seed_matrix.sh"
SCENARIOS = (
    "authoritative_v2_genesis_commits_on_every_validator",
    "authoritative_v2_finalizes_through_validator_restart",
    "taira_npos_leader_timeout_commits_within_rotation_bound",
    "real_network_same_subject_locked_reproposal_converges_after_ordered_quorum_release",
    "real_network_distinct_subject_prepare_qcs_converge_after_causal_release",
)
IGNORED_SCENARIOS: frozenset[str] = frozenset()
SOURCE_MANIFEST = "a" * 64
HEAD_COMMIT = "1" * 40
HEAD_TREE = "2" * 40
CARGO_LOCK_SHA256 = "3" * 64
_EXTERNAL_ROOTS: list[Path] = []


@atexit.register
def _cleanup_external_roots() -> None:
    for root in _EXTERNAL_ROOTS:
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
        (ROOT_DIR / "Cargo.lock").read_bytes()
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
    run_mode: str = "pass",
) -> tuple[dict[str, str], Path, Path]:
    external_root = Path(
        tempfile.mkdtemp(prefix="iroha-seed-matrix-test-", dir="/private/tmp")
    )
    _EXTERNAL_ROOTS.append(external_root)
    cargo_target = external_root / "target"
    artifact_root = external_root / "artifacts"
    cargo_target.mkdir(mode=0o700)
    artifact_root.mkdir(mode=0o700)
    program_target = (
        artifact_root
        / "sumeragi-v2-release"
        / SOURCE_MANIFEST
        / "programs"
        / "invocation.seedmatrixfixture"
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
    capture = tmp_path / "cargo-invocations.tsv"
    cargo = bin_dir / "cargo"
    inventory = "\n".join(
        f"sumeragi_v2_runner::{scenario}: test" for scenario in SCENARIOS
    )
    ignored_inventory = "\n".join(
        f"sumeragi_v2_runner::{scenario}: test" for scenario in IGNORED_SCENARIOS
    )
    cargo.write_text(
        f"""#!/usr/bin/env bash
set -euo pipefail
printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
  "$*" \
  "${{IROHA_TEST_REQUIRE_NETWORK-<unset>}}" \
  "${{IROHA_TEST_NETWORK_START_ATTEMPTS-<unset>}}" \
  "${{IROHA_TEST_NETWORK_BASE_SEED-<unset>}}" \
  "${{TEST_NETWORK_BIN_IROHAD-<unset>}}" \
  "${{IROHA_TEST_SKIP_BUILD-<unset>}}" \
  "${{IROHA_TEST_ALLOW_REENTRANT_BUILD-<unset>}}" \
  "${{IROHA_RELEASE_SOURCE_MANIFEST_SHA256-<unset>}}" \
  "${{CARGO_TARGET_DIR-<unset>}}" \
  "${{IROHA_TEST_TARGET_DIR-<unset>}}" \
  "${{IROHA_TEST_BUILD_TIMEOUT_MS-<unset>}}" \
  "${{IROHA_TEST_PROCESS_TIMEOUT_MS-<unset>}}" \
  "${{IROHA_TEST_NETWORK_PERMIT_WAIT_TIMEOUT-<unset>}}" \
  >>"$SEED_MATRIX_CAPTURE"

case " $* " in
  *" --list --ignored "*)
    printf '%s\n' '{ignored_inventory}'
    exit 0
    ;;
  *" --list "*)
    printf '%s\n' '{inventory}'
    exit 0
    ;;
esac

test_name=""
for arg in "$@"; do
  if [[ "$arg" == sumeragi_v2_runner::* ]]; then
    test_name="$arg"
    break
  fi
done
if [[ -z "$test_name" ]]; then
  exit 66
fi

mkdir -p "$TEST_NETWORK_TMP_DIR/mock_validator"
printf '%s\n' "$test_name" >"$TEST_NETWORK_TMP_DIR/mock_validator/run-1-stdout.log"

emit_success() {{
  scenario="${{test_name#sumeragi_v2_runner::}}"
  printf '%s\n' \
    'running 1 test' \
    "test $test_name ... $scenario: deterministic network seed = $IROHA_TEST_NETWORK_BASE_SEED" \
    'ok' \
    '' \
    'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 42 filtered out; finished in 0.01s' \
    >&2
}}

case "${{SEED_MATRIX_FAKE_RUN_MODE:-pass}}" in
  pass)
    emit_success
    ;;
  hold)
    if [[ ! -e "$SEED_MATRIX_HOLD_STARTED" ]]; then
      : >"$SEED_MATRIX_HOLD_STARTED"
      for ((attempt = 0; attempt < 200; attempt++)); do
        if [[ -e "$SEED_MATRIX_HOLD_RELEASE" ]]; then
          break
        fi
        sleep 0.05
      done
      if [[ ! -e "$SEED_MATRIX_HOLD_RELEASE" ]]; then
        exit 74
      fi
    fi
    emit_success
    ;;
  zero)
    printf '%s\n' \
      'running 0 tests' \
      '' \
      'test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 43 filtered out; finished in 0.00s' \
      >&2
    ;;
  duplicate-summary)
    emit_success
    printf '%s\n' \
      'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 42 filtered out; finished in 0.01s' \
      >&2
    ;;
  wrong-seed)
    scenario="${{test_name#sumeragi_v2_runner::}}"
    printf '%s\n' \
      'running 1 test' \
      "test $test_name ... $scenario: deterministic network seed = wrong-seed" \
      'ok' \
      '' \
      'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 42 filtered out; finished in 0.01s' \
      >&2
    ;;
  cargo-fail)
    emit_success
    exit 73
    ;;
  tamper-bundle)
    emit_success
    chmod 0700 "$SEED_MATRIX_TAMPER_BINARY"
    printf '%s\n' 'tampered after process admission' >"$SEED_MATRIX_TAMPER_BINARY"
    chmod 0500 "$SEED_MATRIX_TAMPER_BINARY"
    ;;
  marker-temp-symlink)
    marker_temp="$TEST_NETWORK_TMP_DIR/../../.COMPLETED.tsv.publish.tmp"
    if [[ ! -e "$marker_temp" && ! -L "$marker_temp" ]]; then
      ln -s "$SEED_MATRIX_ESCAPE_TARGET" "$marker_temp"
    fi
    emit_success
    ;;
  unsafe-symlink)
    ln -s "$SEED_MATRIX_ESCAPE_TARGET" "$TEST_NETWORK_TMP_DIR/mock_validator/escape"
    emit_success
    ;;
  unsafe-special)
    mkfifo "$TEST_NETWORK_TMP_DIR/mock_validator/special"
    emit_success
    ;;
  *)
    exit 64
    ;;
esac
""",
        encoding="utf-8",
    )
    cargo.chmod(0o755)
    marker_failure_harness = tmp_path / "fail-marker-parent-fsync.py"
    marker_failure_harness.write_text(
        """import errno
import importlib.util
import os
from pathlib import Path
import sys

publisher = Path(os.environ["SEED_MATRIX_EXPECTED_MARKER_PUBLISHER"])
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
    python3 = bin_dir / "python3"
    python3.write_text(
        f"""#!/usr/bin/env bash
set -euo pipefail
if [[ $# -eq 3 \
  && "$1" == "scripts/compute_workspace_source_manifest.py" \
  && "$2" == "--root" \
  && "$3" == "$SEED_MATRIX_EXPECTED_REPO_ROOT" ]]; then
  printf '%s\n' '{SOURCE_MANIFEST}'
  exit 0
fi
if [[ "${{1-}}" == "scripts/seal_workspace_source.py" ]]; then
  printf '%s\n' "$*" >>"$SEED_MATRIX_SEAL_CAPTURE"
  exit 0
fi
if [[ "${{1-}}" == "scripts/sumeragi_v2_localnet_manifest.py" \
  || "${{1-}}" == */scripts/sumeragi_v2_prebuilt_bundle.py \
  || ( "${{1-}}" == "-I" \
    && "${{2-}}" == "-S" \
    && ( "${{3-}}" == "-c" || "${{3-}}" == "-" ) ) \
  || ( "${{1-}}" == "-I" \
    && "${{2-}}" == "-S" \
    && "${{3-}}" == */scripts/sumeragi_v2_prebuilt_bundle.py ) ]]; then
  exec "$SEED_MATRIX_REAL_PYTHON3" "$@"
fi
if [[ "${{1-}}" == "-I" \
  && "${{2-}}" == "-S" \
  && "${{3-}}" == */scripts/publish_release_marker.py ]]; then
  if [[ "${{SEED_MATRIX_FAIL_MARKER_PUBLISH:-0}}" == 1 ]]; then
    exec "$SEED_MATRIX_REAL_PYTHON3" \
      "$SEED_MATRIX_MARKER_FAILURE_HARNESS" "${{@:4}}"
  fi
  exec "$SEED_MATRIX_REAL_PYTHON3" "$@"
fi
printf 'unexpected mocked python3 invocation: %s\n' "$*" >&2
exit 65
""",
        encoding="utf-8",
    )
    python3.chmod(0o755)
    env = os.environ.copy()
    # The production parent exports its real digest before invoking this mocked
    # preflight; each stub must instead bind itself to SOURCE_MANIFEST.
    env.pop("IROHA_RELEASE_SOURCE_MANIFEST_SHA256", None)
    env["PATH"] = f"{bin_dir}:{env['PATH']}"
    env["SEED_MATRIX_EXPECTED_REPO_ROOT"] = str(ROOT_DIR)
    env["SEED_MATRIX_CAPTURE"] = str(capture)
    env["SEED_MATRIX_FAKE_RUN_MODE"] = run_mode
    env["SEED_MATRIX_REAL_PYTHON3"] = sys.executable
    env["SEED_MATRIX_EXPECTED_MARKER_PUBLISHER"] = str(
        ROOT_DIR / "scripts" / "publish_release_marker.py"
    )
    env["SEED_MATRIX_MARKER_FAILURE_HARNESS"] = str(marker_failure_harness)
    env["SEED_MATRIX_SEAL_CAPTURE"] = str(tmp_path / "seal-invocations.log")
    env["CARGO_TARGET_DIR"] = str(cargo_target)
    env["IROHA_RELEASE_ARTIFACT_ROOT"] = str(artifact_root)
    env["IROHA_RELEASE_CANCEL_REQUEST_PATH"] = str(
        external_root / "cancel-request.json"
    )
    env["IROHA_TEST_TARGET_DIR"] = str(program_target)
    env["IROHA_RELEASE_PREBUILT_MANIFEST_SHA256"] = manifest_sha256
    evidence = artifact_root / "mocked-command-evidence"
    env["SUMERAGI_V2_SEED_MATRIX_EVIDENCE_DIR"] = str(evidence)
    return env, capture, evidence


def _run_launcher(
    env: dict[str, str], mode: str = "--pr"
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [str(SCRIPT), mode],
        cwd=ROOT_DIR,
        env=env,
        check=False,
        capture_output=True,
        text=True,
    )


def _invocations(evidence_root: Path) -> list[Path]:
    return sorted(path for path in evidence_root.glob("invocation.*") if path.is_dir())


def _single_invocation(evidence_root: Path) -> Path:
    invocations = _invocations(evidence_root)
    assert len(invocations) == 1, invocations
    return invocations[0]


def _summary_rows(invocation: Path) -> list[dict[str, str]]:
    with (invocation / "summary.tsv").open(encoding="utf-8", newline="") as source:
        return list(csv.DictReader(source, delimiter="\t"))


def _key_values(path: Path) -> dict[str, str]:
    return dict(
        line.split("\t", 1)
        for line in path.read_text(encoding="utf-8").splitlines()
    )


def _expected_seed_command(
    source_manifest: str,
    scenario: str,
    seed: str,
    run_index: int,
    cargo_target: Path,
    program_target: Path,
) -> str:
    manifest_sha256 = hashlib.sha256(
        (program_target / ".sumeragi-v2-prebuilt-binaries.tsv").read_bytes()
    ).hexdigest()
    return (
        f"CARGO_TARGET_DIR={cargo_target} "
        f"IROHA_TEST_TARGET_DIR={program_target} "
        f"IROHA_RELEASE_SOURCE_MANIFEST_SHA256={source_manifest} "
        f"IROHA_RELEASE_PREBUILT_MANIFEST_SHA256={manifest_sha256} "
        f"TEST_NETWORK_BIN_IROHAD={program_target / 'release' / 'iroha3d'} "
        "TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL="
        f"{program_target / 'message-control' / 'release' / 'iroha3d'} "
        f"TEST_NETWORK_BIN_IROHA={program_target / 'release' / 'iroha'} "
        f"KAGAMI_BIN={program_target / 'release' / 'kagami'} "
        "CARGO_NET_OFFLINE=true "
        "IROHA_TEST_REQUIRE_NETWORK=1 "
        "IROHA_TEST_NETWORK_START_ATTEMPTS=1 "
        "IROHA_TEST_SKIP_BUILD=1 "
        "IROHA_TEST_ALLOW_REENTRANT_BUILD=0 "
        "IROHA_TEST_BUILD_PROFILE=release "
        "PROFILE=release "
        "IROHA_TEST_BUILD_TIMEOUT_MS=3600 "
        "IROHA_TEST_PROCESS_TIMEOUT_MS=300 "
        "IROHA_TEST_NETWORK_PERMIT_WAIT_TIMEOUT=300 "
        f"IROHA_TEST_NETWORK_BASE_SEED={seed} "
        "TEST_NETWORK_TMP_DIR=${SEED_MATRIX_EVIDENCE_DIRECTORY}/"
        f"localnets/run-{run_index:03d} "
        "IROHA_TEST_NETWORK_KEEP_DIRS=1 "
        "cargo test --locked --offline -p integration_tests --test "
        "sumeragi_v2_runner_isolated "
        f"sumeragi_v2_runner::{scenario} -- --exact --nocapture "
        "--test-threads=1"
    )


def _wait_for_path(path: Path, timeout_seconds: float = 10.0) -> None:
    deadline = time.monotonic() + timeout_seconds
    while time.monotonic() < deadline:
        if path.exists():
            return
        time.sleep(0.01)
    raise AssertionError(f"timed out waiting for {path}")


def test_mocked_seed_matrix_runs_every_exact_scenario_with_one_start_attempt(
    tmp_path: Path,
) -> None:
    env, capture, evidence = _stubbed_environment(tmp_path)
    env["IROHA_TEST_REQUIRE_NETWORK"] = "inherited-unsafe"
    env["IROHA_TEST_NETWORK_START_ATTEMPTS"] = "99"
    env["TEST_NETWORK_BIN_IROHAD"] = "/tmp/inherited-stale-iroha3d"
    env["IROHA_TEST_SKIP_BUILD"] = "1"
    env["IROHA_TEST_ALLOW_REENTRANT_BUILD"] = "0"
    env["IROHA_RELEASE_SEALED_WORKTREE"] = "1"
    completion_pointer = (
        Path(env["IROHA_RELEASE_ARTIFACT_ROOT"]) / "seed-completion-path"
    )
    env["IROHA_SEED_MATRIX_COMPLETION_PATH_FILE"] = str(completion_pointer)

    result = _run_launcher(env)

    assert result.returncode == 0, result.stderr
    rows = [line.split("\t") for line in capture.read_text().splitlines()]
    assert len(rows) == 2 + len(SCENARIOS) * 4
    execution_rows = rows[2:]
    assert len(execution_rows) == len(SCENARIOS) * 4
    assert all(
        "-- --exact --nocapture --test-threads=1" in row[0]
        for row in execution_rows
    )
    for row, scenario in zip(
        execution_rows,
        (scenario for scenario in SCENARIOS for _ in range(4)),
    ):
        assert (" --ignored" in row[0]) == (scenario in IGNORED_SCENARIOS)
    assert all(row[1:3] == ["1", "1"] for row in rows)
    expected_program_target = Path(env["IROHA_TEST_TARGET_DIR"])
    assert all(
        row[4:7]
        == [str(expected_program_target / "release" / "iroha3d"), "1", "0"]
        for row in rows
    )
    source_manifests = {row[7] for row in rows}
    assert source_manifests == {SOURCE_MANIFEST}
    source_manifest = SOURCE_MANIFEST
    expected_source_root = (
        Path(env["IROHA_RELEASE_ARTIFACT_ROOT"])
        / "sumeragi-v2-release"
        / source_manifest
    )
    expected_cargo_target = Path(env["CARGO_TARGET_DIR"])
    assert all(row[8] == str(expected_cargo_target) for row in rows)
    assert all(row[9] == str(expected_program_target) for row in rows)
    assert all(row[10:] == ["3600", "300", "300"] for row in rows)
    expected_seeds = [
        seed
        for scenario in SCENARIOS
        for seed in (
            scenario,
            f"{scenario}:seed:01",
            f"{scenario}:seed:02",
            f"{scenario}:seed:03",
        )
    ]
    assert [row[3] for row in execution_rows] == expected_seeds
    assert result.stdout.count("running 1 test") == len(execution_rows)
    invocation = _single_invocation(evidence)
    summary_rows = _summary_rows(invocation)
    assert len(summary_rows) == len(execution_rows)
    assert [row["scenario"] for row in summary_rows] == [
        scenario for scenario in SCENARIOS for _ in range(4)
    ]
    assert [row["seed"] for row in summary_rows] == expected_seeds
    assert {row["result"] for row in summary_rows} == {"passed"}
    assert {row["source_manifest_sha256"] for row in summary_rows} == {
        source_manifest
    }
    assert {row["cargo_status"] for row in summary_rows} == {"0"}
    assert {row["tee_status"] for row in summary_rows} == {"0"}
    for index, row in enumerate(summary_rows):
        output = invocation / row["output"]
        localnet = invocation / row["localnet"]
        assert output.is_file()
        assert localnet.is_dir()
        retained_logs = list(localnet.glob("*/run-1-stdout.log"))
        assert len(retained_logs) == 1
        assert retained_logs[0].read_text(encoding="utf-8").strip().startswith(
            "sumeragi_v2_runner::"
        )
        assert "running 1 test" in output.read_text(encoding="utf-8")
        assert row["run_log_sha256"] == hashlib.sha256(output.read_bytes()).hexdigest()
        assert row["command"] == _expected_seed_command(
            source_manifest,
            row["scenario"],
            row["seed"],
            index,
            expected_cargo_target,
            expected_program_target,
        )
        assert str(invocation) not in row["command"]
        assert (" --ignored" in row["command"]) == (
            row["scenario"] in IGNORED_SCENARIOS
        )
        manifest_relative = f"localnet-manifests/run-{index:03d}.tsv"
        manifest = invocation / manifest_relative
        assert manifest.is_file()
        retained_log = retained_logs[0]
        assert manifest.read_text(encoding="utf-8") == (
            "path\tsize_bytes\tsha256\n"
            f"mock_validator/run-1-stdout.log\t{retained_log.stat().st_size}\t"
            f"{hashlib.sha256(retained_log.read_bytes()).hexdigest()}\n"
        )
    invocation_fields = _key_values(invocation / "invocation.tsv")
    assert invocation_fields == {
        "schema_version": "1",
        "profile": "pr",
        "source_manifest_sha256": source_manifest,
        "source_bound_root": str(expected_source_root),
        "cargo_target_dir": str(expected_cargo_target),
        "iroha_test_target_dir": str(expected_program_target),
        "prebuilt_manifest_sha256": env[
            "IROHA_RELEASE_PREBUILT_MANIFEST_SHA256"
        ],
        "expected_runs": str(len(SCENARIOS) * 4),
        "build_timeout_seconds": "3600",
        "process_timeout_seconds": "300",
        "network_permit_wait_timeout_seconds": "300",
        "process_lifetime_enforcement": "internal_deadlines_no_outer_process_signal",
        "completion_file": "COMPLETED.tsv",
    }
    completion_fields = _key_values(invocation / "COMPLETED.tsv")
    assert completion_fields["schema_version"] == "2"
    assert completion_fields["profile"] == "pr"
    assert completion_fields["source_manifest_sha256"] == source_manifest
    assert (
        completion_fields["prebuilt_manifest_sha256"]
        == env["IROHA_RELEASE_PREBUILT_MANIFEST_SHA256"]
    )
    assert completion_fields["completed_runs"] == str(len(SCENARIOS) * 4)
    assert completion_fields["expected_runs"] == str(len(SCENARIOS) * 4)
    assert completion_fields["summary_sha256"] == hashlib.sha256(
        (invocation / "summary.tsv").read_bytes()
    ).hexdigest()
    assert completion_fields["localnet_manifest_count"] == str(
        len(SCENARIOS) * 4
    )
    assert completion_fields["localnet_manifests_path"] == "localnet-manifests.tsv"
    manifest_index = invocation / completion_fields["localnet_manifests_path"]
    assert completion_fields["localnet_manifests_sha256"] == hashlib.sha256(
        manifest_index.read_bytes()
    ).hexdigest()
    with manifest_index.open(encoding="utf-8", newline="") as source:
        manifest_rows = list(csv.DictReader(source, delimiter="\t"))
    assert len(manifest_rows) == len(SCENARIOS) * 4
    for index, manifest_row in enumerate(manifest_rows):
        relative = f"localnet-manifests/run-{index:03d}.tsv"
        digest = hashlib.sha256((invocation / relative).read_bytes()).hexdigest()
        assert manifest_row == {
            "run_index": str(index),
            "localnet": f"localnets/run-{index:03d}",
            "manifest": relative,
            "manifest_sha256": digest,
        }
        assert completion_fields[f"localnet_manifest_{index:03d}_path"] == relative
        assert completion_fields[f"localnet_manifest_{index:03d}_sha256"] == digest
    assert str(invocation / "summary.tsv") in result.stderr
    assert str(invocation / "COMPLETED.tsv") in result.stderr
    assert completion_pointer.read_text(encoding="utf-8").strip() == str(
        invocation / "COMPLETED.tsv"
    )
    seal_invocations = Path(env["SEED_MATRIX_SEAL_CAPTURE"]).read_text(
        encoding="utf-8"
    )
    assert "--verify --root" in seal_invocations
    assert "--no-writable-paths" in seal_invocations
    assert "--writable target" not in seal_invocations
    assert not (evidence / ".seed-matrix.lock").exists()


def test_mocked_seed_matrix_preserves_prior_invocation_evidence(
    tmp_path: Path,
) -> None:
    env, _, evidence = _stubbed_environment(tmp_path)
    prior_invocation = evidence / "invocation.previous"
    stale_run = prior_invocation / "runs" / "run-999.log"
    stale_localnet = (
        prior_invocation / "localnets" / "run-999" / "old-validator"
    )
    stale_localnet.mkdir(parents=True)
    stale_run.parent.mkdir(parents=True)
    stale_run.write_text("old release run\n", encoding="utf-8")
    (stale_localnet / "run-1-stdout.log").write_text(
        "old release validator\n", encoding="utf-8"
    )

    result = _run_launcher(env)

    assert result.returncode == 0, result.stderr
    assert stale_run.read_text(encoding="utf-8") == "old release run\n"
    assert (stale_localnet / "run-1-stdout.log").read_text(
        encoding="utf-8"
    ) == "old release validator\n"
    invocations = _invocations(evidence)
    assert prior_invocation in invocations
    assert len(invocations) == 2
    current = next(path for path in invocations if path != prior_invocation)
    assert len(list((current / "runs").glob("run-*.log"))) == len(SCENARIOS) * 4
    assert len(list((current / "localnets").glob("run-*"))) == len(SCENARIOS) * 4
    assert (current / "COMPLETED.tsv").is_file()


def test_mocked_seed_matrix_release_profile_uses_32_seeds_per_scenario(
    tmp_path: Path,
) -> None:
    env, capture, evidence = _stubbed_environment(tmp_path)
    env["IROHA_RELEASE_HEAD_COMMIT"] = HEAD_COMMIT
    env["IROHA_RELEASE_HEAD_TREE"] = HEAD_TREE
    env["IROHA_RELEASE_CARGO_LOCK_SHA256"] = CARGO_LOCK_SHA256

    result = _run_launcher(env, "--release")

    assert result.returncode == 0, result.stderr
    assert len(SCENARIOS) == 5
    assert len(capture.read_text().splitlines()) == 162
    invocation = _single_invocation(evidence)
    summary_rows = _summary_rows(invocation)
    assert len(summary_rows) == 160
    assert {row["profile"] for row in summary_rows} == {"release"}
    assert {row["result"] for row in summary_rows} == {"passed"}
    assert all(
        row["command"]
        == _expected_seed_command(
            SOURCE_MANIFEST,
            row["scenario"],
            row["seed"],
            index,
            Path(env["CARGO_TARGET_DIR"]),
            Path(env["IROHA_TEST_TARGET_DIR"]),
        )
        for index, row in enumerate(summary_rows)
    )
    for scenario_index, scenario in enumerate(SCENARIOS):
        scenario_rows = summary_rows[
            scenario_index * 32 : (scenario_index + 1) * 32
        ]
        assert scenario_rows[0]["seed"] == scenario
        assert scenario_rows[-1]["seed"] == f"{scenario}:seed:31"
    completion = _key_values(invocation / "COMPLETED.tsv")
    assert completion["completed_runs"] == "160"
    assert completion["expected_runs"] == "160"
    assert completion["head_commit"] == HEAD_COMMIT
    assert completion["head_tree"] == HEAD_TREE
    assert completion["cargo_lock_sha256"] == CARGO_LOCK_SHA256
    assert completion["localnet_manifest_count"] == "160"


def test_mocked_seed_matrix_rejects_zero_test_and_preserves_evidence(
    tmp_path: Path,
) -> None:
    env, capture, evidence = _stubbed_environment(tmp_path, run_mode="zero")

    result = _run_launcher(env)

    assert result.returncode == 1
    assert "refusing zero-test, wrong-seed, or ambiguous Cargo success" in result.stderr
    assert "running 0 tests" in result.stdout
    assert len(capture.read_text().splitlines()) == 3
    invocation = _single_invocation(evidence)
    summary_rows = _summary_rows(invocation)
    assert len(summary_rows) == 1
    assert summary_rows[0]["result"] == "invalid_output"
    output = invocation / summary_rows[0]["output"]
    assert output.is_file()
    assert "running 0 tests" in output.read_text(encoding="utf-8")
    localnet = invocation / summary_rows[0]["localnet"]
    assert localnet.is_dir()
    assert str(output) in result.stderr
    assert str(localnet) in result.stderr
    assert not (invocation / "COMPLETED.tsv").exists()
    assert not (evidence / ".seed-matrix.lock").exists()


def test_mocked_seed_matrix_rejects_ambiguous_test_summary(
    tmp_path: Path,
) -> None:
    ambiguous_root = tmp_path / "ambiguous"
    ambiguous_root.mkdir()
    env, capture, evidence = _stubbed_environment(
        ambiguous_root, run_mode="duplicate-summary"
    )

    result = _run_launcher(env)

    assert result.returncode == 1
    assert "refusing zero-test, wrong-seed, or ambiguous Cargo success" in result.stderr
    assert len(capture.read_text().splitlines()) == 3
    invocation = _single_invocation(evidence)
    summary_rows = _summary_rows(invocation)
    assert len(summary_rows) == 1
    assert summary_rows[0]["result"] == "invalid_output"
    assert (invocation / summary_rows[0]["output"]).is_file()
    assert (invocation / summary_rows[0]["localnet"]).is_dir()
    assert not (invocation / "COMPLETED.tsv").exists()

    wrong_seed_root = tmp_path / "wrong-seed"
    wrong_seed_root.mkdir()
    wrong_env, wrong_capture, wrong_evidence = _stubbed_environment(
        wrong_seed_root, run_mode="wrong-seed"
    )

    wrong_result = _run_launcher(wrong_env)

    assert wrong_result.returncode == 1
    assert "wrong-seed" in wrong_result.stderr
    assert len(wrong_capture.read_text().splitlines()) == 3
    wrong_invocation = _single_invocation(wrong_evidence)
    wrong_rows = _summary_rows(wrong_invocation)
    assert len(wrong_rows) == 1
    assert wrong_rows[0]["result"] == "invalid_output"
    assert not (wrong_invocation / "COMPLETED.tsv").exists()


def test_mocked_seed_matrix_preserves_cargo_failure_through_tee(
    tmp_path: Path,
) -> None:
    env, capture, evidence = _stubbed_environment(tmp_path, run_mode="cargo-fail")

    result = _run_launcher(env)

    assert result.returncode == 1
    assert "seed-matrix test command failed" in result.stderr
    assert "cargo=73, tee=0" in result.stderr
    assert len(capture.read_text().splitlines()) == 3
    invocation = _single_invocation(evidence)
    summary_rows = _summary_rows(invocation)
    assert len(summary_rows) == 1
    assert summary_rows[0]["result"] == "command_failed"
    assert summary_rows[0]["cargo_status"] == "73"
    assert summary_rows[0]["tee_status"] == "0"
    output = invocation / summary_rows[0]["output"]
    assert output.is_file()
    assert "test result: ok. 1 passed" in output.read_text(encoding="utf-8")
    localnet = invocation / summary_rows[0]["localnet"]
    assert localnet.is_dir()
    assert str(output) in result.stderr
    assert str(localnet) in result.stderr
    assert not (invocation / "COMPLETED.tsv").exists()


def test_mocked_seed_matrix_rejects_bundle_tampering_before_completion(
    tmp_path: Path,
) -> None:
    env, _capture, evidence = _stubbed_environment(
        tmp_path,
        run_mode="tamper-bundle",
    )
    program_target = Path(env["IROHA_TEST_TARGET_DIR"])
    binary = program_target / "release" / "iroha3d"
    original = binary.read_bytes()
    env["SEED_MATRIX_TAMPER_BINARY"] = str(binary)
    completion_pointer = (
        Path(env["IROHA_RELEASE_ARTIFACT_ROOT"]) / "seed-completion-path"
    )
    env["IROHA_SEED_MATRIX_COMPLETION_PATH_FILE"] = str(completion_pointer)
    try:
        result = _run_launcher(env)
    finally:
        binary.chmod(0o700)
        binary.write_bytes(original)
        binary.chmod(0o500)

    assert result.returncode == 1
    assert "binary bundle changed before seed-matrix completion" in result.stderr
    invocation = _single_invocation(evidence)
    assert not (invocation / "COMPLETED.tsv").exists()
    assert not completion_pointer.exists()


def test_mocked_seed_matrix_rejects_symlinked_marker_temp_without_completion(
    tmp_path: Path,
) -> None:
    env, _capture, evidence = _stubbed_environment(
        tmp_path,
        run_mode="marker-temp-symlink",
    )
    escape = tmp_path / "marker-temp-escape"
    escape.write_text("must remain unchanged\n", encoding="utf-8")
    env["SEED_MATRIX_ESCAPE_TARGET"] = str(escape)
    completion_pointer = (
        Path(env["IROHA_RELEASE_ARTIFACT_ROOT"]) / "seed-completion-path"
    )
    env["IROHA_SEED_MATRIX_COMPLETION_PATH_FILE"] = str(completion_pointer)

    result = _run_launcher(env)

    assert result.returncode == 1
    assert "completion marker temporary already exists as symlink" in result.stderr
    invocation = _single_invocation(evidence)
    marker_temp = invocation / ".COMPLETED.tsv.publish.tmp"
    assert marker_temp.is_symlink()
    assert escape.read_text(encoding="utf-8") == "must remain unchanged\n"
    assert not (invocation / "COMPLETED.tsv").exists()
    assert not completion_pointer.exists()


def test_mocked_seed_matrix_marker_durability_failure_is_not_terminal(
    tmp_path: Path,
) -> None:
    env, _capture, evidence = _stubbed_environment(tmp_path)
    env["SEED_MATRIX_FAIL_MARKER_PUBLISH"] = "1"
    completion_pointer = (
        Path(env["IROHA_RELEASE_ARTIFACT_ROOT"]) / "seed-completion-path"
    )
    env["IROHA_SEED_MATRIX_COMPLETION_PATH_FILE"] = str(completion_pointer)

    result = _run_launcher(env)

    assert result.returncode == 1
    assert "mocked completion-parent fsync failure" in result.stderr
    invocation = _single_invocation(evidence)
    assert not (invocation / "COMPLETED.tsv").exists()
    assert not completion_pointer.exists()


def test_mocked_seed_matrix_rejects_parent_source_manifest_mismatch(
    tmp_path: Path,
) -> None:
    env, capture, evidence = _stubbed_environment(tmp_path)
    env["IROHA_RELEASE_SOURCE_MANIFEST_SHA256"] = "b" * 64

    result = _run_launcher(env)

    assert result.returncode == 1
    assert "source manifest does not match the parent release invocation" in result.stderr
    assert SOURCE_MANIFEST in result.stderr
    assert "b" * 64 in result.stderr
    assert not capture.exists()
    assert not evidence.exists()


def test_mocked_seed_matrix_rejects_source_drift_before_completion(
    tmp_path: Path,
) -> None:
    env, capture, evidence = _stubbed_environment(tmp_path)
    manifest_counter = tmp_path / "manifest-count"
    python3 = Path(env["PATH"].split(os.pathsep, 1)[0]) / "python3"
    python3.write_text(
        f"""#!/usr/bin/env bash
set -euo pipefail
if [[ "${{1-}}" == */scripts/sumeragi_v2_prebuilt_bundle.py \
  || "${{1-}}" == "scripts/sumeragi_v2_localnet_manifest.py" \
  || ( "${{1-}}" == "-I" \
    && "${{2-}}" == "-S" \
    && ( "${{3-}}" == "-c" || "${{3-}}" == "-" ) ) \
  || ( "${{1-}}" == "-I" \
    && "${{2-}}" == "-S" \
    && "${{3-}}" == */scripts/sumeragi_v2_prebuilt_bundle.py ) ]]; then
  exec "$SEED_MATRIX_REAL_PYTHON3" "$@"
fi
if [[ $# -ne 3 \
  || "$1" != "scripts/compute_workspace_source_manifest.py" \
  || "$2" != "--root" \
  || "$3" != "$SEED_MATRIX_EXPECTED_REPO_ROOT" ]]; then
  exit 65
fi
count=0
if [[ -f "$SEED_MATRIX_MANIFEST_COUNTER" ]]; then
  count="$(<"$SEED_MATRIX_MANIFEST_COUNTER")"
fi
count=$((count + 1))
printf '%s\n' "$count" >"$SEED_MATRIX_MANIFEST_COUNTER"
if ((count <= 4)); then
  printf '%s\n' '{SOURCE_MANIFEST}'
else
  printf '%s\n' '{"b" * 64}'
fi
""",
        encoding="utf-8",
    )
    python3.chmod(0o755)
    env["SEED_MATRIX_MANIFEST_COUNTER"] = str(manifest_counter)
    env["IROHA_RELEASE_SOURCE_MANIFEST_SHA256"] = SOURCE_MANIFEST
    completion_pointer = (
        Path(env["IROHA_RELEASE_ARTIFACT_ROOT"]) / "seed-completion-path"
    )
    env["IROHA_SEED_MATRIX_COMPLETION_PATH_FILE"] = str(completion_pointer)

    result = _run_launcher(env)

    assert result.returncode == 1
    assert "workspace sources changed during the seed matrix" in result.stderr
    assert "after authoritative_v2_genesis_commits_on_every_validator" in result.stderr
    assert len(capture.read_text(encoding="utf-8").splitlines()) == 3
    invocation = _single_invocation(evidence)
    rows = _summary_rows(invocation)
    assert len(rows) == 1
    assert rows[0]["result"] == "source_changed"
    assert rows[0]["source_manifest_sha256"] == SOURCE_MANIFEST
    assert not (invocation / "COMPLETED.tsv").exists()
    assert not completion_pointer.exists()
    assert not (evidence / ".seed-matrix.lock").exists()


def test_mocked_seed_matrix_rejects_concurrent_writer_without_clobbering(
    tmp_path: Path,
) -> None:
    first_root = tmp_path / "first"
    second_root = tmp_path / "second"
    first_root.mkdir()
    second_root.mkdir()
    first_env, first_capture, _ = _stubbed_environment(first_root, run_mode="hold")
    second_env, second_capture, _ = _stubbed_environment(second_root)
    evidence = Path(first_env["IROHA_RELEASE_ARTIFACT_ROOT"]) / "shared-evidence"
    hold_started = tmp_path / "hold-started"
    hold_release = tmp_path / "hold-release"
    first_env["SUMERAGI_V2_SEED_MATRIX_EVIDENCE_DIR"] = str(evidence)
    first_env["SEED_MATRIX_HOLD_STARTED"] = str(hold_started)
    first_env["SEED_MATRIX_HOLD_RELEASE"] = str(hold_release)
    second_env["SUMERAGI_V2_SEED_MATRIX_EVIDENCE_DIR"] = str(evidence)
    for inherited_name in (
        "CARGO_TARGET_DIR",
        "IROHA_RELEASE_ARTIFACT_ROOT",
        "IROHA_RELEASE_CANCEL_REQUEST_PATH",
        "IROHA_TEST_TARGET_DIR",
        "IROHA_RELEASE_PREBUILT_MANIFEST_SHA256",
    ):
        second_env[inherited_name] = first_env[inherited_name]

    first = subprocess.Popen(
        [str(SCRIPT), "--pr"],
        cwd=ROOT_DIR,
        env=first_env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    try:
        _wait_for_path(hold_started)
        in_progress = _single_invocation(evidence)
        assert not (in_progress / "COMPLETED.tsv").exists()

        second = _run_launcher(second_env)

        assert second.returncode == 1
        assert "another seed-matrix invocation owns" in second.stderr
        assert "refusing shared evidence" in second.stderr
        assert not second_capture.exists()
        assert _invocations(evidence) == [in_progress]
        assert not (in_progress / "COMPLETED.tsv").exists()
    finally:
        hold_release.write_text("continue\n", encoding="utf-8")
        first_stdout, first_stderr = first.communicate(timeout=30)

    assert first.returncode == 0, (first_stdout, first_stderr)
    assert len(first_capture.read_text(encoding="utf-8").splitlines()) == 2 + len(SCENARIOS) * 4
    assert (in_progress / "COMPLETED.tsv").is_file()
    assert not (evidence / ".seed-matrix.lock").exists()


def test_mocked_seed_matrix_refuses_uninspected_stale_lock(
    tmp_path: Path,
) -> None:
    env, capture, evidence = _stubbed_environment(tmp_path)

    partial_env = env.copy()
    partial_env.pop("IROHA_RELEASE_ARTIFACT_ROOT")
    partial_env.pop("IROHA_RELEASE_CANCEL_REQUEST_PATH")
    partial = _run_launcher(partial_env)
    assert partial.returncode == 2
    assert "must be supplied all-or-none" in partial.stderr
    assert not evidence.exists()

    escaped_env = env.copy()
    escaped = tmp_path / "escaped-evidence"
    escaped_env["SUMERAGI_V2_SEED_MATRIX_EVIDENCE_DIR"] = str(escaped)
    escaped_result = _run_launcher(escaped_env)
    assert escaped_result.returncode == 2
    assert "escapes its authenticated root" in escaped_result.stderr
    assert not escaped.exists()

    cancel_path = Path(env["IROHA_RELEASE_CANCEL_REQUEST_PATH"])
    cancel_path.write_text(
        '{"reason":"operator-request","schema_version":1}\n',
        encoding="utf-8",
    )
    cancel_path.chmod(0o600)
    cancelled = _run_launcher(env)
    assert cancelled.returncode == 125
    assert "seed-matrix:entry" in cancelled.stderr
    assert not evidence.exists()
    cancel_path.unlink()

    lock = evidence / ".seed-matrix.lock"
    lock.mkdir(parents=True)
    (lock / "owner").write_text(
        "pid=123\nprofile=pr\nsource_manifest_sha256=" + SOURCE_MANIFEST + "\n",
        encoding="utf-8",
    )

    result = _run_launcher(env)

    assert result.returncode == 1
    assert "another seed-matrix invocation owns" in result.stderr
    assert not capture.exists()
    assert _invocations(evidence) == []
    assert (lock / "owner").is_file()


def test_mocked_seed_matrix_rejects_unsafe_retained_localnet_entries(
    tmp_path: Path,
) -> None:
    for run_mode, expected_error in (
        ("unsafe-symlink", "contains a symlink"),
        ("unsafe-special", "non-regular special file"),
    ):
        case_root = tmp_path / run_mode
        case_root.mkdir()
        escape_target = tmp_path / f"{run_mode}-outside"
        escape_target.write_text("outside retained localnet\n", encoding="utf-8")
        env, capture, evidence = _stubbed_environment(
            case_root, run_mode=run_mode
        )
        env["SEED_MATRIX_ESCAPE_TARGET"] = str(escape_target)

        result = _run_launcher(env)

        assert result.returncode == 1
        assert expected_error in result.stderr
        assert "retained localnet" in result.stderr
        assert len(capture.read_text(encoding="utf-8").splitlines()) == 3
        invocation = _single_invocation(evidence)
        rows = _summary_rows(invocation)
        assert len(rows) == 1
        assert rows[0]["result"] == "invalid_localnet"
        assert not (invocation / "COMPLETED.tsv").exists()
        assert not (evidence / ".seed-matrix.lock").exists()
