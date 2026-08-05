"""Mocked-Cargo contract tests for the Nexus cross-dataspace launcher.

These tests exercise shell-level release environment handling only. They do
not execute Cargo, compile Rust, or provide real-network release evidence.
"""

from __future__ import annotations

import os
import subprocess
from pathlib import Path

import pytest


ROOT_DIR = Path(__file__).resolve().parents[2]
SCRIPT = ROOT_DIR / "scripts" / "run_nexus_cross_dataspace_atomic_swap.sh"
HEAD_COMMIT = "1" * 40
HEAD_TREE = "2" * 40
SOURCE_MANIFEST = "3" * 64
CARGO_LOCK = "4" * 64
PREBUILT_MANIFEST = "5" * 64
AUTOSCALE = (
    "nexus::autoscale_localnet::"
    "nexus_autoscale_four_peer_release_lifecycle_recreates_lane_and_rejects_stale_artifacts"
)
AUTOSCALE_RESTART = (
    "nexus::autoscale_localnet::"
    "nexus_autoscale_certified_merge_recovers_missing_sidecar_after_restart"
)
AUTOSCALE_DRAIN = (
    "nexus::autoscale_localnet::"
    "nexus_autoscale_two_phase_drain_closes_certifies_then_retires_after_restart"
)
NATIVE_AMX = (
    "native_amx_rotating_validator_fault_soak_preserves_independent_participant_qcs"
)
EX297_IDLE = (
    "sumeragi_localnet_smoke::"
    "permissioned_idle_chain_advances_only_for_external_or_internal_work"
)
EX297_PHASE_CUT = (
    "musubi_selectable_publication_phase_cut_matrix_is_atomic_after_replay"
)
RELEASE_TESTS = (
    AUTOSCALE,
    AUTOSCALE_RESTART,
    AUTOSCALE_DRAIN,
    NATIVE_AMX,
    EX297_IDLE,
    EX297_PHASE_CUT,
)


def _fixture(tmp_path: Path) -> tuple[dict[str, str], Path, Path, Path]:
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()

    ps = bin_dir / "ps"
    ps.write_text(
        "#!/usr/bin/env bash\n"
        "set -euo pipefail\n"
        "[[ \"$*\" == '-axo pid,etime,command' ]] || exit 64\n"
        "printf '%s\\n' '  PID ELAPSED COMMAND'\n",
        encoding="utf-8",
    )
    ps.chmod(0o755)

    capture = tmp_path / "cargo-environment.log"
    cargo = bin_dir / "cargo"
    cargo.write_text(
        f"""#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "${{IROHA_NATIVE_AMX_SOAK_ITERATIONS-<unset>}}" \
  >>"$NEXUS_LAUNCHER_CAPTURE"

target=""
previous=""
for arg in "$@"; do
  if [[ "$previous" == "--test" ]]; then
    target="$arg"
    break
  fi
  previous="$arg"
done

case " $* " in
  *" --list --ignored "*) exit 0 ;;
  *" --list "*)
    case "$target" in
      nexus_and_streaming)
        printf '%s\n' \
          '{AUTOSCALE}: test' \
          '{AUTOSCALE_RESTART}: test' \
          '{AUTOSCALE_DRAIN}: test'
        ;;
      native_amx_routing)
        printf '%s\n' '{NATIVE_AMX}: test' '{EX297_PHASE_CUT}: test'
        ;;
      consensus_and_da)
        printf '%s\n' '{EX297_IDLE}: test'
        ;;
      *) exit 65 ;;
    esac
    exit 0
    ;;
esac

test_name=""
for arg in "$@"; do
  case "$arg" in
    '{AUTOSCALE}'|'{AUTOSCALE_RESTART}'|'{AUTOSCALE_DRAIN}'|\
    '{NATIVE_AMX}'|'{EX297_IDLE}'|'{EX297_PHASE_CUT}')
      test_name="$arg"
      break
      ;;
  esac
done
[[ -n "$test_name" ]] || exit 66

printf '%s\n' 'running 1 test'
case "$test_name" in
  '{AUTOSCALE}'|'{NATIVE_AMX}'|'{EX297_IDLE}'|'{EX297_PHASE_CUT}')
    printf '%s\n' "[multilane-release-gate] started: $test_name"
    ;;
esac
case "$test_name" in
  '{NATIVE_AMX}')
    printf '%s\n' '[multilane-release-native-evidence] grouped_sources=2 durable_manifest=passed body_eviction_recovery=passed authenticated_remote_recovery=passed exact_once=passed'
    ;;
  '{EX297_IDLE}')
    printf '%s\n' '[ex-297-idle-evidence] clean_idle=passed external_non_empty=passed internal_non_empty=passed'
    ;;
  '{EX297_PHASE_CUT}')
    printf '%s\n' '[ex-297-phase-cut-evidence] after_prepare_qc=passed after_commit_qc=passed before_world_commit=passed exact_once=passed'
    ;;
esac
case "$test_name" in
  '{AUTOSCALE}'|'{NATIVE_AMX}'|'{EX297_IDLE}'|'{EX297_PHASE_CUT}')
    printf '%s\n' "[multilane-release-gate] completed: $test_name"
    ;;
esac
printf '%s\n' \
  "test $test_name ... ok" \
  '' \
  'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.01s'
""",
        encoding="utf-8",
    )
    cargo.chmod(0o755)

    evidence = tmp_path / "evidence"
    pointer = tmp_path / "completion-path"
    permits = tmp_path / "permits"
    permits.mkdir()
    env = os.environ.copy()
    env.update(
        {
            "PATH": f"{bin_dir}:{env['PATH']}",
            "NEXUS_LAUNCHER_CAPTURE": str(capture),
            "IROHA_RELEASE_HEAD_COMMIT": HEAD_COMMIT,
            "IROHA_RELEASE_HEAD_TREE": HEAD_TREE,
            "IROHA_RELEASE_SOURCE_MANIFEST_SHA256": SOURCE_MANIFEST,
            "IROHA_RELEASE_CARGO_LOCK_SHA256": CARGO_LOCK,
            "IROHA_RELEASE_PREBUILT_MANIFEST_SHA256": PREBUILT_MANIFEST,
            "IROHA_MULTILANE_FOUR_PEER_COMPLETION_PATH_FILE": str(pointer),
            "IROHA_TEST_NETWORK_PERMIT_DIR": str(permits),
        }
    )
    return env, evidence, pointer, capture


def _run(
    env: dict[str, str],
    evidence: Path,
    *extra_args: str,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            str(SCRIPT),
            "--release",
            "--capture",
            "--evidence-dir",
            str(evidence),
            "--multilane-four-peer-release",
            *extra_args,
        ],
        cwd=ROOT_DIR,
        env=env,
        check=False,
        capture_output=True,
        text=True,
    )


@pytest.mark.parametrize("inherited_value", ["1", "100", "invalid"])
def test_multilane_release_pins_native_amx_iterations(
    tmp_path: Path,
    inherited_value: str,
) -> None:
    env, evidence, pointer, capture = _fixture(tmp_path)
    env["IROHA_NATIVE_AMX_SOAK_ITERATIONS"] = inherited_value

    result = _run(env, evidence)

    assert result.returncode == 0, result.stderr
    assert pointer.is_file()
    completion = Path(pointer.read_text(encoding="utf-8").strip())
    assert completion.is_file()
    assert "passed_runs\t6\n" in completion.read_text(encoding="utf-8")
    observed = capture.read_text(encoding="utf-8").splitlines()
    assert len(observed) == len(RELEASE_TESTS) * 3
    assert set(observed) == {"10"}


def test_multilane_release_rejects_iteration_env_override_before_cargo(
    tmp_path: Path,
) -> None:
    env, evidence, pointer, capture = _fixture(tmp_path)

    result = _run(
        env,
        evidence,
        "--env",
        "IROHA_NATIVE_AMX_SOAK_ITERATIONS=1",
    )

    assert result.returncode == 2
    assert (
        "--env may not override reserved cross-dataspace evidence control "
        "IROHA_NATIVE_AMX_SOAK_ITERATIONS"
    ) in result.stderr
    assert not capture.exists()
    assert not pointer.exists()
