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


def _fixture(
    tmp_path: Path, *, run_mode: str = "pass", drift_after: int = 0
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
      'test accelerated_100_000_block_chaos_preserves_chain_prefix ... SUMERAGI_V2_CHAOS_COMPLETED permissioned_heights=50000 npos_heights=50000 total_heights=100000' \\
      'ok' \\
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
  fail) exit 73 ;;
esac
""",
        encoding="utf-8",
    )
    harness.chmod(0o755)

    identity = json.dumps(
        {
            "schema_version": 1,
            "head_commit": HEAD,
            "head_tree": TREE,
            "index_tree": TREE,
            "workspace_source_manifest_sha256": MANIFEST,
            "cargo_lock_sha256": LOCK,
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
      printf '%s\n' '{identity.replace(MANIFEST, "b" * 64)}'
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
    env["IROHA_RELEASE_SOURCE_MANIFEST_SHA256"] = MANIFEST
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
    assert fields["permissioned_heights"] == "50000"
    assert fields["npos_heights"] == "50000"
    assert fields["completed_heights"] == "100000"
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


def test_chaos_launcher_refuses_stale_writer_lock(tmp_path: Path) -> None:
    launcher, env, evidence = _fixture(tmp_path)
    lock = evidence / ".chaos-100k.lock"
    lock.mkdir(parents=True)

    result = _run(launcher, env)

    assert result.returncode == 1
    assert "another 100,000-height chaos gate owns" in result.stderr
    assert list(evidence.glob("invocation.*")) == []
