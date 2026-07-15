"""Mocked-Cargo contract tests for the Sumeragi v2 deterministic seed runner.

These tests exercise the shell launcher's accounting and failure handling; they
are not evidence that any real validator process started or made progress.
"""

from __future__ import annotations

import csv
import os
import subprocess
from pathlib import Path


ROOT_DIR = Path(__file__).resolve().parents[2]
SCRIPT = ROOT_DIR / "scripts" / "run_sumeragi_v2_seed_matrix.sh"
SCENARIOS = (
    "authoritative_v2_genesis_commits_on_every_validator",
    "authoritative_v2_finalizes_through_validator_restart",
    "taira_npos_leader_timeout_commits_within_rotation_bound",
    "real_network_divergent_prepare_qcs_converge_after_ordered_release",
)


def _stubbed_environment(
    tmp_path: Path,
    *,
    run_mode: str = "pass",
) -> tuple[dict[str, str], Path, Path]:
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    capture = tmp_path / "cargo-invocations.tsv"
    cargo = bin_dir / "cargo"
    inventory = "\n".join(
        f"sumeragi_v2_runner::{scenario}: test" for scenario in SCENARIOS
    )
    cargo.write_text(
        f"""#!/usr/bin/env bash
set -euo pipefail
printf '%s\t%s\t%s\t%s\n' \
  "$*" \
  "${{IROHA_TEST_REQUIRE_NETWORK-<unset>}}" \
  "${{IROHA_TEST_NETWORK_START_ATTEMPTS-<unset>}}" \
  "${{IROHA_TEST_NETWORK_BASE_SEED-<unset>}}" \
  >>"$SEED_MATRIX_CAPTURE"

case " $* " in
  *" --list --ignored "*)
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

emit_success() {{
  printf '%s\n' \
    'running 1 test' \
    "test $test_name ... ok" \
    '' \
    'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 42 filtered out; finished in 0.01s' \
    >&2
}}

case "${{SEED_MATRIX_FAKE_RUN_MODE:-pass}}" in
  pass)
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
  cargo-fail)
    emit_success
    exit 73
    ;;
  *)
    exit 64
    ;;
esac
""",
        encoding="utf-8",
    )
    cargo.chmod(0o755)
    env = os.environ.copy()
    env["PATH"] = f"{bin_dir}:{env['PATH']}"
    env["SEED_MATRIX_CAPTURE"] = str(capture)
    env["SEED_MATRIX_FAKE_RUN_MODE"] = run_mode
    evidence = tmp_path / "mocked-command-evidence"
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


def _summary_rows(evidence: Path) -> list[dict[str, str]]:
    with (evidence / "summary.tsv").open(encoding="utf-8", newline="") as source:
        return list(csv.DictReader(source, delimiter="\t"))


def test_mocked_seed_matrix_runs_every_exact_scenario_with_one_start_attempt(
    tmp_path: Path,
) -> None:
    env, capture, evidence = _stubbed_environment(tmp_path)
    env["IROHA_TEST_REQUIRE_NETWORK"] = "inherited-unsafe"
    env["IROHA_TEST_NETWORK_START_ATTEMPTS"] = "99"

    result = _run_launcher(env)

    assert result.returncode == 0, result.stderr
    rows = [line.split("\t") for line in capture.read_text().splitlines()]
    assert len(rows) == 18
    execution_rows = rows[2:]
    assert len(execution_rows) == len(SCENARIOS) * 4
    assert all(
        "-- --exact --nocapture --test-threads=1" in row[0]
        for row in execution_rows
    )
    assert all(row[1:3] == ["1", "1"] for row in rows)
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
    summary_rows = _summary_rows(evidence)
    assert len(summary_rows) == len(execution_rows)
    assert [row["scenario"] for row in summary_rows] == [
        scenario for scenario in SCENARIOS for _ in range(4)
    ]
    assert [row["seed"] for row in summary_rows] == expected_seeds
    assert {row["result"] for row in summary_rows} == {"passed"}
    assert {row["cargo_status"] for row in summary_rows} == {"0"}
    assert {row["tee_status"] for row in summary_rows} == {"0"}
    assert {row["localnet"] for row in summary_rows} == {"-"}
    for row in summary_rows:
        output = evidence / row["output"]
        assert output.is_file()
        assert "running 1 test" in output.read_text(encoding="utf-8")
        assert row["seed"] in row["command"]
        assert "--exact --nocapture --test-threads=1" in row["command"]
    assert str(evidence / "summary.tsv") in result.stderr


def test_mocked_seed_matrix_release_profile_uses_32_seeds_per_scenario(
    tmp_path: Path,
) -> None:
    env, capture, evidence = _stubbed_environment(tmp_path)

    result = _run_launcher(env, "--release")

    assert result.returncode == 0, result.stderr
    assert len(capture.read_text().splitlines()) == 2 + len(SCENARIOS) * 32
    summary_rows = _summary_rows(evidence)
    assert len(summary_rows) == len(SCENARIOS) * 32
    assert {row["profile"] for row in summary_rows} == {"release"}
    assert {row["result"] for row in summary_rows} == {"passed"}
    for scenario_index, scenario in enumerate(SCENARIOS):
        scenario_rows = summary_rows[
            scenario_index * 32 : (scenario_index + 1) * 32
        ]
        assert scenario_rows[0]["seed"] == scenario
        assert scenario_rows[-1]["seed"] == f"{scenario}:seed:31"


def test_mocked_seed_matrix_rejects_zero_test_and_preserves_evidence(
    tmp_path: Path,
) -> None:
    env, capture, evidence = _stubbed_environment(tmp_path, run_mode="zero")

    result = _run_launcher(env)

    assert result.returncode == 1
    assert "refusing zero-test or ambiguous Cargo success" in result.stderr
    assert "running 0 tests" in result.stdout
    assert len(capture.read_text().splitlines()) == 3
    summary_rows = _summary_rows(evidence)
    assert len(summary_rows) == 1
    assert summary_rows[0]["result"] == "invalid_output"
    output = evidence / summary_rows[0]["output"]
    assert output.is_file()
    assert "running 0 tests" in output.read_text(encoding="utf-8")
    localnet = evidence / summary_rows[0]["localnet"]
    assert localnet.is_dir()
    assert str(output) in result.stderr
    assert str(localnet) in result.stderr


def test_mocked_seed_matrix_rejects_ambiguous_test_summary(
    tmp_path: Path,
) -> None:
    env, capture, evidence = _stubbed_environment(
        tmp_path, run_mode="duplicate-summary"
    )

    result = _run_launcher(env)

    assert result.returncode == 1
    assert "refusing zero-test or ambiguous Cargo success" in result.stderr
    assert len(capture.read_text().splitlines()) == 3
    summary_rows = _summary_rows(evidence)
    assert len(summary_rows) == 1
    assert summary_rows[0]["result"] == "invalid_output"
    assert (evidence / summary_rows[0]["output"]).is_file()
    assert (evidence / summary_rows[0]["localnet"]).is_dir()


def test_mocked_seed_matrix_preserves_cargo_failure_through_tee(
    tmp_path: Path,
) -> None:
    env, capture, evidence = _stubbed_environment(tmp_path, run_mode="cargo-fail")

    result = _run_launcher(env)

    assert result.returncode == 1
    assert "seed-matrix test command failed" in result.stderr
    assert "cargo=73, tee=0" in result.stderr
    assert len(capture.read_text().splitlines()) == 3
    summary_rows = _summary_rows(evidence)
    assert len(summary_rows) == 1
    assert summary_rows[0]["result"] == "command_failed"
    assert summary_rows[0]["cargo_status"] == "73"
    assert summary_rows[0]["tee_status"] == "0"
    output = evidence / summary_rows[0]["output"]
    assert output.is_file()
    assert "test result: ok. 1 passed" in output.read_text(encoding="utf-8")
    localnet = evidence / summary_rows[0]["localnet"]
    assert localnet.is_dir()
    assert str(output) in result.stderr
    assert str(localnet) in result.stderr
