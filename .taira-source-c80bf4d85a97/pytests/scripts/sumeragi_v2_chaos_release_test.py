"""Mocked release-evidence tests for the 100,000-height chaos launcher."""

from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import subprocess

ROOT_DIR = Path(__file__).resolve().parents[2]
SCRIPT = ROOT_DIR / "scripts" / "run_sumeragi_v2_100k_chaos.sh"
MANIFEST = "a" * 64
HEAD = "1" * 40
TREE = "2" * 40
LOCK = "3" * 64
CHAOS_MARKER = (
    "SUMERAGI_V2_CHAOS_COMPLETED permissioned_heights=50000 "
    "npos_heights=50000 total_heights=100000 supplied_commit_qcs=100000 "
    "supplied_tcs=75000 finalized_validators=400000 wal_append_restarts=314 "
    "fetch_restarts=312 store_restarts=312 validation_restarts=312 "
    "application_restarts=312 stale_generation_rejections=1562 "
    "deferred_fetch_completions=400936 deferred_store_completions=400624 "
    "deferred_validation_completions=400312 "
    "deferred_application_completions=400000 duplicate_commit_qcs=3124 "
    "reordered_commit_batches=75000 reordered_tc_batches=75000 "
    "insufficient_dual_qcs=1030 count_only_qcs=515 power_only_qcs=515 "
    "restart_interval=64 duplicate_interval=32 under_quorum_interval=97 "
    "certificate_source=external_fixture"
)
CHAOS_FIELDS = {
    "schema_version": "2",
    "permissioned_heights": "50000",
    "npos_heights": "50000",
    "completed_heights": "100000",
    "supplied_commit_qcs": "100000",
    "supplied_tcs": "75000",
    "finalized_validators": "400000",
    "wal_append_restarts": "314",
    "fetch_restarts": "312",
    "store_restarts": "312",
    "validation_restarts": "312",
    "application_restarts": "312",
    "stale_generation_rejections": "1562",
    "deferred_fetch_completions": "400936",
    "deferred_store_completions": "400624",
    "deferred_validation_completions": "400312",
    "deferred_application_completions": "400000",
    "duplicate_commit_qcs": "3124",
    "reordered_commit_batches": "75000",
    "reordered_tc_batches": "75000",
    "insufficient_dual_qcs": "1030",
    "count_only_qcs": "515",
    "power_only_qcs": "515",
    "restart_interval": "64",
    "duplicate_interval": "32",
    "under_quorum_interval": "97",
    "certificate_source": "external_fixture",
}


def _fixture(
    tmp_path: Path,
    *,
    run_mode: str = "pass",
    drift_after: int = 0,
    manifest: str = MANIFEST,
    head: str = HEAD,
    tree: str = TREE,
    lock: str = LOCK,
) -> tuple[Path, dict[str, str], Path]:
    repo = tmp_path / "repo"
    scripts = repo / "scripts"
    formal = scripts / "formal"
    bin_dir = tmp_path / "bin"
    formal.mkdir(parents=True)
    bin_dir.mkdir()
    launcher = scripts / SCRIPT.name
    shutil.copy2(SCRIPT, launcher)

    harness = formal / "run_sumeragi_v2_harness.sh"
    harness.write_text(
        f"""#!/usr/bin/env bash
set -euo pipefail
case "${{CHAOS_FAKE_RUN_MODE:-pass}}" in
  pass)
    printf '%s\n' \\
      'running 1 test' \\
      '{CHAOS_MARKER}' \\
      'test accelerated_100_000_block_chaos_preserves_chain_prefix ... ok' \\
      '' \\
      'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out; finished in 0.01s'
    ;;
  zero)
    printf '%s\n' 'running 0 tests' \\
      'test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 0.00s'
    ;;
  missing-marker)
    printf '%s\n' \\
      'running 1 test' \\
      'test accelerated_100_000_block_chaos_preserves_chain_prefix ... ok' \\
      '' \\
      'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out; finished in 0.01s'
    ;;
  wrong-counters)
    printf '%s\n' \\
      'running 1 test' \\
      '{CHAOS_MARKER.replace("wal_append_restarts=314", "wal_append_restarts=315")}' \\
      'test accelerated_100_000_block_chaos_preserves_chain_prefix ... ok' \\
      '' \\
      'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out; finished in 0.01s'
    ;;
  duplicate-marker)
    printf '%s\n' \\
      'running 1 test' \\
      '{CHAOS_MARKER}' \\
      '{CHAOS_MARKER}' \\
      'test accelerated_100_000_block_chaos_preserves_chain_prefix ... ok' \\
      '' \\
      'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out; finished in 0.01s'
    ;;
  duplicate-test-line)
    printf '%s\n' \\
      'running 1 test' \\
      '{CHAOS_MARKER}' \\
      'test accelerated_100_000_block_chaos_preserves_chain_prefix ... ok' \\
      'test accelerated_100_000_block_chaos_preserves_chain_prefix ... ok' \\
      '' \\
      'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out; finished in 0.01s'
    ;;
  fail) exit 73 ;;
esac
""",
        encoding="utf-8",
    )
    harness.chmod(0o755)

    identity = json.dumps(
        {
            "schema_version": 1,
            "head_commit": head,
            "head_tree": tree,
            "index_tree": tree,
            "workspace_source_manifest_sha256": manifest,
            "cargo_lock_sha256": lock,
        },
        sort_keys=True,
        separators=(",", ":"),
    )
    counter = tmp_path / "identity-count"
    python = bin_dir / "python3"
    python.write_text(
        f"""#!/usr/bin/env bash
set -euo pipefail
case "${{1:-}}" in
  *compute_workspace_source_manifest.py)
    count=0
    [[ ! -f "$CHAOS_IDENTITY_COUNTER" ]] || count="$(<"$CHAOS_IDENTITY_COUNTER")"
    count=$((count + 1))
    printf '%s\n' "$count" >"$CHAOS_IDENTITY_COUNTER"
    if ((CHAOS_DRIFT_AFTER > 0 && count > CHAOS_DRIFT_AFTER)); then
      printf '%s\n' '{identity.replace(manifest, "f" * 64)}'
    else
      printf '%s\n' '{identity}'
    fi
    ;;
  *seal_workspace_source.py) exit 0 ;;
  *) exec /usr/bin/python3 "$@" ;;
esac
""",
        encoding="utf-8",
    )
    python.chmod(0o755)

    env = os.environ.copy()
    env["PATH"] = f"{bin_dir}:{env['PATH']}"
    env["CHAOS_FAKE_RUN_MODE"] = run_mode
    env["CHAOS_IDENTITY_COUNTER"] = str(counter)
    env["CHAOS_DRIFT_AFTER"] = str(drift_after)
    env["IROHA_RELEASE_SOURCE_MANIFEST_SHA256"] = manifest
    evidence = tmp_path / "evidence"
    env["SUMERAGI_V2_CHAOS_EVIDENCE_DIR"] = str(evidence)
    return launcher, env, evidence


def _run(launcher: Path, env: dict[str, str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [str(launcher)],
        cwd=launcher.parents[1],
        env=env,
        check=False,
        capture_output=True,
        text=True,
    )


def _invocation(evidence: Path) -> Path:
    invocations = list(evidence.glob("invocation.*"))
    assert len(invocations) == 1
    return invocations[0]


def test_chaos_launcher_publishes_source_bound_completion(tmp_path: Path) -> None:
    launcher, env, evidence = _fixture(tmp_path)
    pointer = tmp_path / "chaos-completion-path"
    env["IROHA_CHAOS_COMPLETION_PATH_FILE"] = str(pointer)

    result = _run(launcher, env)

    assert result.returncode == 0, result.stderr
    invocation = _invocation(evidence)
    completion = invocation / "COMPLETED.tsv"
    assert completion.is_file()
    assert pointer.read_text(encoding="utf-8").strip() == str(completion)
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    assert fields["head_commit"] == HEAD
    assert fields["head_tree"] == TREE
    assert fields["source_manifest_sha256"] == MANIFEST
    assert fields["cargo_lock_sha256"] == LOCK
    for field, expected in CHAOS_FIELDS.items():
        assert fields[field] == expected
    assert not (evidence / ".chaos-100k.lock").exists()


def test_chaos_launcher_rejects_post_run_identity_drift(tmp_path: Path) -> None:
    launcher, env, evidence = _fixture(tmp_path, drift_after=2)
    pointer = tmp_path / "chaos-completion-path"
    env["IROHA_CHAOS_COMPLETION_PATH_FILE"] = str(pointer)

    result = _run(launcher, env)

    assert result.returncode == 1
    assert "source identity changed at after execution" in result.stderr
    assert not (_invocation(evidence) / "COMPLETED.tsv").exists()
    assert not pointer.exists()


def test_chaos_launcher_rejects_zero_test_success(tmp_path: Path) -> None:
    zero_root = tmp_path / "zero"
    zero_root.mkdir()
    launcher, env, evidence = _fixture(zero_root, run_mode="zero")

    result = _run(launcher, env)

    assert result.returncode == 1
    assert "does not prove exactly one passing release test" in result.stderr
    assert not (_invocation(evidence) / "COMPLETED.tsv").exists()

    marker_root = tmp_path / "missing-marker"
    marker_root.mkdir()
    marker_launcher, marker_env, marker_evidence = _fixture(
        marker_root, run_mode="missing-marker"
    )

    marker_result = _run(marker_launcher, marker_env)

    assert marker_result.returncode == 1
    assert "does not prove exactly one passing release test" in marker_result.stderr
    assert not (_invocation(marker_evidence) / "COMPLETED.tsv").exists()

    counters_root = tmp_path / "wrong-counters"
    counters_root.mkdir()
    counters_launcher, counters_env, counters_evidence = _fixture(
        counters_root, run_mode="wrong-counters"
    )

    counters_result = _run(counters_launcher, counters_env)

    assert counters_result.returncode == 1
    assert "does not prove exactly one passing release test" in counters_result.stderr
    assert not (_invocation(counters_evidence) / "COMPLETED.tsv").exists()


def test_chaos_launcher_rejects_duplicate_completion_evidence(tmp_path: Path) -> None:
    for run_mode in ("duplicate-marker", "duplicate-test-line"):
        root = tmp_path / run_mode
        root.mkdir()
        launcher, env, evidence = _fixture(root, run_mode=run_mode)

        result = _run(launcher, env)

        assert result.returncode == 1
        assert "does not prove exactly one passing release test" in result.stderr
        assert not (_invocation(evidence) / "COMPLETED.tsv").exists()


def test_chaos_launcher_refuses_stale_writer_lock(tmp_path: Path) -> None:
    launcher, env, evidence = _fixture(tmp_path)
    lock = evidence / ".chaos-100k.lock"
    lock.mkdir(parents=True)

    result = _run(launcher, env)

    assert result.returncode == 1
    assert "another 100,000-height chaos gate owns" in result.stderr
    assert list(evidence.glob("invocation.*")) == []
