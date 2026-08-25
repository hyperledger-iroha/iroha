"""Mocked release-evidence tests for the 100,000-height chaos launcher."""

from __future__ import annotations

import atexit
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

ROOT_DIR = Path(__file__).resolve().parents[2]
SCRIPT = ROOT_DIR / "scripts" / "run_sumeragi_v2_100k_chaos.sh"
PROCESS_POLICY = ROOT_DIR / "scripts" / "sumeragi_v2_release_process_policy.sh"
MARKER_PUBLISHER = ROOT_DIR / "scripts" / "publish_release_marker.py"
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
    "insufficient_dual_qcs=1030 count_only_qcs=0 power_only_qcs=0 "
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
    "count_only_qcs": "0",
    "power_only_qcs": "0",
    "restart_interval": "64",
    "duplicate_interval": "32",
    "under_quorum_interval": "97",
    "certificate_source": "external_fixture",
}
_EXTERNAL_ROOTS: list[Path] = []


@atexit.register
def _cleanup_external_roots() -> None:
    for root in _EXTERNAL_ROOTS:
        shutil.rmtree(root, ignore_errors=True)


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
    shutil.copy2(PROCESS_POLICY, scripts / PROCESS_POLICY.name)
    shutil.copy2(MARKER_PUBLISHER, scripts / MARKER_PUBLISHER.name)

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
      'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 11 filtered out; finished in 0.01s'
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
      'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 11 filtered out; finished in 0.01s'
    ;;
  wrong-counters)
    printf '%s\n' \\
      'running 1 test' \\
      '{CHAOS_MARKER.replace("wal_append_restarts=314", "wal_append_restarts=315")}' \\
      'test accelerated_100_000_block_chaos_preserves_chain_prefix ... ok' \\
      '' \\
      'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 11 filtered out; finished in 0.01s'
    ;;
  duplicate-marker)
    printf '%s\n' \\
      'running 1 test' \\
      '{CHAOS_MARKER}' \\
      '{CHAOS_MARKER}' \\
      'test accelerated_100_000_block_chaos_preserves_chain_prefix ... ok' \\
      '' \\
      'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 11 filtered out; finished in 0.01s'
    ;;
  duplicate-test-line)
    printf '%s\n' \\
      'running 1 test' \\
      '{CHAOS_MARKER}' \\
      'test accelerated_100_000_block_chaos_preserves_chain_prefix ... ok' \\
      'test accelerated_100_000_block_chaos_preserves_chain_prefix ... ok' \\
      '' \\
      'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 11 filtered out; finished in 0.01s'
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
  *seal_workspace_source.py)
    printf '%s\n' "$*" >>"$CHAOS_SEAL_CAPTURE"
    exit 0
    ;;
  *) exec "$CHAOS_REAL_PYTHON3" "$@" ;;
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
    env["CHAOS_REAL_PYTHON3"] = sys.executable
    env["CHAOS_SEAL_CAPTURE"] = str(tmp_path / "seal-invocations.log")
    env["IROHA_RELEASE_SOURCE_MANIFEST_SHA256"] = manifest
    external_root = Path(
        tempfile.mkdtemp(prefix="iroha-chaos-launcher-test-", dir="/private/tmp")
    )
    _EXTERNAL_ROOTS.append(external_root)
    target = external_root / "target"
    artifacts = external_root / "artifacts"
    target.mkdir(mode=0o700)
    artifacts.mkdir(mode=0o700)
    env["CARGO_TARGET_DIR"] = str(target)
    env["IROHA_RELEASE_ARTIFACT_ROOT"] = str(artifacts)
    env["IROHA_RELEASE_CANCEL_REQUEST_PATH"] = str(
        external_root / "cancel-request.json"
    )
    evidence = artifacts / "evidence"
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
    pointer = Path(env["IROHA_RELEASE_ARTIFACT_ROOT"]) / "chaos-completion-path"
    env["IROHA_CHAOS_COMPLETION_PATH_FILE"] = str(pointer)
    env["IROHA_RELEASE_SEALED_WORKTREE"] = "1"

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
    seal_invocations = Path(env["CHAOS_SEAL_CAPTURE"]).read_text(encoding="utf-8")
    assert "--verify --root" in seal_invocations
    assert "--no-writable-paths" in seal_invocations
    assert "--writable target" not in seal_invocations
    assert not (evidence / ".chaos-100k.lock").exists()


def test_chaos_launcher_rejects_post_run_identity_drift(tmp_path: Path) -> None:
    launcher, env, evidence = _fixture(tmp_path, drift_after=2)
    pointer = Path(env["IROHA_RELEASE_ARTIFACT_ROOT"]) / "chaos-completion-path"
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

    partial_env = env.copy()
    partial_env.pop("IROHA_RELEASE_ARTIFACT_ROOT")
    partial_env.pop("IROHA_RELEASE_CANCEL_REQUEST_PATH")
    partial = _run(launcher, partial_env)
    assert partial.returncode == 2
    assert "must be supplied all-or-none" in partial.stderr
    assert not evidence.exists()

    escaped_env = env.copy()
    escaped = tmp_path / "escaped-evidence"
    escaped_env["SUMERAGI_V2_CHAOS_EVIDENCE_DIR"] = str(escaped)
    escaped_result = _run(launcher, escaped_env)
    assert escaped_result.returncode == 2
    assert "escapes its authenticated root" in escaped_result.stderr
    assert not escaped.exists()

    cancel_path = Path(env["IROHA_RELEASE_CANCEL_REQUEST_PATH"])
    cancel_path.write_text(
        '{"reason":"operator-request","schema_version":1}\n',
        encoding="utf-8",
    )
    cancel_path.chmod(0o600)
    cancelled = _run(launcher, env)
    assert cancelled.returncode == 125
    assert "chaos-100k:entry" in cancelled.stderr
    assert not evidence.exists()
    cancel_path.unlink()

    lock = evidence / ".chaos-100k.lock"
    lock.mkdir(parents=True)

    result = _run(launcher, env)

    assert result.returncode == 1
    assert "another 100,000-height chaos gate owns" in result.stderr
    assert list(evidence.glob("invocation.*")) == []
