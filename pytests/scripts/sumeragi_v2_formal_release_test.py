"""Mocked release-evidence tests for the strict formal launcher."""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

import pytest

ROOT_DIR = Path(__file__).resolve().parents[2]
SCRIPT = ROOT_DIR / "scripts" / "run_sumeragi_v2_formal_release.sh"
JAVA_RESOLVER = ROOT_DIR / "scripts" / "formal" / "resolve_java.sh"
MANIFEST = "a" * 64
HEAD = "1" * 40
TREE = "2" * 40
LOCK = "3" * 64
FINAL_MARKER = (
    "Sumeragi v2 formal gate passed: source-bound TLAPS, adversarial scheduler "
    "mutations, bounded TLC, trace replay, and production Verus"
)


def _write_fake_java(path: Path, *, working: bool) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "#!/usr/bin/env bash\n"
        "[[ \"${1:-}\" == -version ]] || exit 64\n"
        f"exit {0 if working else 65}\n",
        encoding="utf-8",
    )
    path.chmod(0o755)


def _fixture(
    tmp_path: Path,
    *,
    gate_mode: str = "pass",
    drift_after: int = 0,
    checker_status: int = 0,
) -> tuple[Path, dict[str, str], Path]:
    repo = tmp_path / "repo"
    scripts = repo / "scripts"
    formal = scripts / "formal"
    ci = repo / "ci"
    bin_dir = tmp_path / "bin"
    formal.mkdir(parents=True)
    ci.mkdir()
    bin_dir.mkdir()
    launcher = scripts / SCRIPT.name
    shutil.copy2(SCRIPT, launcher)
    shutil.copy2(JAVA_RESOLVER, formal / JAVA_RESOLVER.name)
    shutil.copy2(
        ROOT_DIR / "scripts" / "formal" / "sumeragi_v2_harness.lock",
        formal / "sumeragi_v2_harness.lock",
    )

    checker = formal / "check_sumeragi_v2_proof_ledger.py"
    checker.write_text("raise SystemExit(99)\n", encoding="utf-8")
    gate = ci / "check_sumeragi_formal.sh"
    gate.write_text(
        f"""#!/usr/bin/env bash
set -euo pipefail
mkdir -p docs/formal/sumeragi_v2 target/formal/sumeragi_v2
case "${{FORMAL_FAKE_GATE_MODE:-pass}}" in
  fail) exit 73 ;;
  pass|no-marker|duplicate-marker)
    printf '%s\n' '{{"machine_checked_completion":true}}' \
      >docs/formal/sumeragi_v2/proof_coverage.json
    printf '%s\n' '{{"backend_verification":true}}' \
      >target/formal/sumeragi_v2/proof_evidence.json
    ;;
esac
case "${{FORMAL_FAKE_GATE_MODE:-pass}}" in
  pass) printf '%s\n' '{FINAL_MARKER}' ;;
  no-marker) printf '%s\n' 'formal legs ended without marker' ;;
  duplicate-marker) printf '%s\n' '{FINAL_MARKER}' '{FINAL_MARKER}' ;;
esac
""",
        encoding="utf-8",
    )
    gate.chmod(0o755)

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
    [[ ! -f "$FORMAL_IDENTITY_COUNTER" ]] || count="$(<"$FORMAL_IDENTITY_COUNTER")"
    count=$((count + 1))
    printf '%s\n' "$count" >"$FORMAL_IDENTITY_COUNTER"
    if ((FORMAL_DRIFT_AFTER > 0 && count > FORMAL_DRIFT_AFTER)); then
      printf '%s\n' '{identity.replace(MANIFEST, "b" * 64)}'
    else
      printf '%s\n' '{identity}'
    fi
    ;;
  *check_sumeragi_v2_proof_ledger.py) exit "$FORMAL_CHECKER_STATUS" ;;
  *seal_workspace_source.py) exit 0 ;;
  *) exec /usr/bin/python3 "$@" ;;
esac
""",
        encoding="utf-8",
    )
    python.chmod(0o755)
    tools: dict[str, Path] = {}
    for name in ("java", "tlapm", "tla2tools.jar", "verus", "cargo-verus"):
        tool = bin_dir / name
        if name == "java":
            _write_fake_java(tool, working=True)
        else:
            tool.write_text(f"fixture {name}\n", encoding="utf-8")
            tool.chmod(0o755)
        tools[name] = tool

    env = os.environ.copy()
    env["PATH"] = f"{bin_dir}:{env['PATH']}"
    env["FORMAL_FAKE_GATE_MODE"] = gate_mode
    env["FORMAL_IDENTITY_COUNTER"] = str(counter)
    env["FORMAL_DRIFT_AFTER"] = str(drift_after)
    env["FORMAL_CHECKER_STATUS"] = str(checker_status)
    env["IROHA_RELEASE_SOURCE_MANIFEST_SHA256"] = MANIFEST
    env["JAVA_BIN"] = str(tools["java"])
    env["TLAPM_BIN"] = str(tools["tlapm"])
    env["TLA2TOOLS_JAR"] = str(tools["tla2tools.jar"])
    evidence = tmp_path / "evidence"
    env["SUMERAGI_V2_FORMAL_EVIDENCE_DIR"] = str(evidence)
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


def _fields(path: Path) -> dict[str, str]:
    return dict(
        line.split("\t", 1) for line in path.read_text(encoding="utf-8").splitlines()
    )


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def test_formal_launcher_publishes_complete_source_bound_archive(
    tmp_path: Path,
) -> None:
    launcher, env, evidence = _fixture(tmp_path)
    pointer = tmp_path / "formal-completion-path"
    env["IROHA_FORMAL_COMPLETION_PATH_FILE"] = str(pointer)

    result = _run(launcher, env)

    assert result.returncode == 0, result.stderr
    invocation = _invocation(evidence)
    completion = invocation / "COMPLETED.tsv"
    fields = _fields(completion)
    assert fields["head_commit"] == HEAD
    assert fields["head_tree"] == TREE
    assert fields["source_manifest_sha256"] == MANIFEST
    assert fields["cargo_lock_sha256"] == LOCK
    assert fields["formal_gate_log_sha256"] == _sha256(invocation / "formal-gate.log")
    assert fields["proof_coverage_sha256"] == _sha256(
        invocation / "proof_coverage.json"
    )
    assert fields["proof_evidence_sha256"] == _sha256(
        invocation / "proof_evidence.json"
    )
    assert fields["harness_cargo_lock_sha256"] == _sha256(
        invocation / "harness-Cargo.lock"
    )
    assert fields["formal_toolchain_sha256"] == _sha256(
        invocation / "formal-toolchain.tsv"
    )
    assert (invocation / "formal-gate.log").read_text(encoding="utf-8").splitlines()[
        -1
    ] == FINAL_MARKER
    assert pointer.read_text(encoding="utf-8").strip() == str(completion)
    assert not (evidence / ".formal-release.lock").exists()


def test_formal_launcher_preserves_pipeline_failure_without_completion(
    tmp_path: Path,
) -> None:
    launcher, env, evidence = _fixture(tmp_path, gate_mode="fail")

    result = _run(launcher, env)

    assert result.returncode == 1
    assert "formal release command failed (gate=73, tee=0)" in result.stderr
    assert not (_invocation(evidence) / "COMPLETED.tsv").exists()


@pytest.mark.parametrize("gate_mode", ["no-marker", "duplicate-marker"])
def test_formal_launcher_requires_one_exact_final_marker(
    tmp_path: Path, gate_mode: str
) -> None:
    launcher, env, evidence = _fixture(tmp_path, gate_mode=gate_mode)

    result = _run(launcher, env)

    assert result.returncode == 1
    assert "one exact final success marker" in result.stderr
    assert not (_invocation(evidence) / "COMPLETED.tsv").exists()


def test_formal_launcher_rejects_post_gate_identity_drift(tmp_path: Path) -> None:
    launcher, env, evidence = _fixture(tmp_path, drift_after=2)

    result = _run(launcher, env)

    assert result.returncode == 1
    assert "source identity changed at after execution" in result.stderr
    assert not (_invocation(evidence) / "COMPLETED.tsv").exists()


def test_formal_launcher_requires_archived_pair_to_pass_release_checker(
    tmp_path: Path,
) -> None:
    launcher, env, evidence = _fixture(tmp_path, checker_status=81)

    result = _run(launcher, env)

    assert result.returncode == 81
    assert not (_invocation(evidence) / "COMPLETED.tsv").exists()


def test_formal_launcher_refuses_stale_writer_lock(tmp_path: Path) -> None:
    launcher, env, evidence = _fixture(tmp_path)
    (evidence / ".formal-release.lock").mkdir(parents=True)

    result = _run(launcher, env)

    assert result.returncode == 1
    assert "another strict formal release gate owns" in result.stderr
    assert list(evidence.glob("invocation.*")) == []


def test_java_resolver_canonicalizes_an_explicit_working_runtime(
    tmp_path: Path,
) -> None:
    runtime = tmp_path / "jdk" / "bin" / "java"
    _write_fake_java(runtime, working=True)
    alias = tmp_path / "java-alias"
    alias.symlink_to(runtime)

    result = subprocess.run(
        [str(JAVA_RESOLVER), str(alias)],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 0, result.stderr
    assert result.stdout.strip() == str(runtime.resolve())


def test_java_resolver_rejects_an_explicit_broken_runtime_without_fallback(
    tmp_path: Path,
) -> None:
    broken = tmp_path / "broken-java"
    fallback = tmp_path / "bin" / "java"
    _write_fake_java(broken, working=False)
    _write_fake_java(fallback, working=True)
    env = os.environ.copy()
    env["PATH"] = f"{fallback.parent}:{env['PATH']}"

    result = subprocess.run(
        [str(JAVA_RESOLVER), str(broken)],
        env=env,
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 1
    assert "configured Java runtime is not a working executable" in result.stderr
    assert result.stdout == ""


def test_java_resolver_skips_a_broken_path_stub_for_repo_local_jdk(
    tmp_path: Path,
) -> None:
    repo = tmp_path / "repo"
    resolver = repo / "scripts" / "formal" / JAVA_RESOLVER.name
    resolver.parent.mkdir(parents=True)
    shutil.copy2(JAVA_RESOLVER, resolver)
    path_stub = tmp_path / "bin" / "java"
    local_java = (
        repo
        / "target"
        / "java"
        / "jdk-21"
        / "Contents"
        / "Home"
        / "bin"
        / "java"
    )
    _write_fake_java(path_stub, working=False)
    _write_fake_java(local_java, working=True)
    env = os.environ.copy()
    env.pop("JAVA_HOME", None)
    env["PATH"] = f"{path_stub.parent}:{env['PATH']}"

    result = subprocess.run(
        [str(resolver)],
        env=env,
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 0, result.stderr
    assert result.stdout.strip() == str(local_java.resolve())


def test_java_resolver_rejects_extra_arguments() -> None:
    result = subprocess.run(
        [str(JAVA_RESOLVER), "java", "unexpected"],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 2
    assert "usage:" in result.stderr


def test_every_sumeragi_formal_java_entrypoint_uses_the_shared_resolver() -> None:
    entrypoints = (
        ROOT_DIR / "ci" / "check_sumeragi_formal.sh",
        ROOT_DIR / "scripts" / "run_sumeragi_v2_formal_release.sh",
        ROOT_DIR / "scripts" / "formal" / "run_sumeragi_v2_tlc.sh",
        ROOT_DIR / "scripts" / "formal" / "check_sumeragi_v2_replay_trace.sh",
        ROOT_DIR / "scripts" / "formal" / "run_sumeragi_v2_service_rank_mutation.sh",
        ROOT_DIR / "scripts" / "formal" / "run_sumeragi_v2_progress_mutations.sh",
    )
    for entrypoint in entrypoints:
        source = entrypoint.read_text(encoding="utf-8")
        assert "resolve_java.sh" in source
        assert '"$JAVA_BIN"' in source

    replay = entrypoints[3].read_text(encoding="utf-8")
    assert "\n  java -XX:+UseParallelGC" not in replay

    release = (
        ROOT_DIR / "scripts" / "run_sumeragi_v2_release_gates.sh"
    ).read_text(encoding="utf-8")
    assert "unset JAVA_BIN" in release
    assert 'release_java_bin="$("$repo_root/scripts/formal/resolve_java.sh")"' in release
    assert "canonical_executable java" not in release
