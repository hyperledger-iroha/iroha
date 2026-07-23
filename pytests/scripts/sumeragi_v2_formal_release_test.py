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
    "Sumeragi v2 formal gate passed: source-bound TLAPS, adversarial "
    "scheduler/post-decision/recovery/effect-capacity/ingress-causal-freshness "
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
    cross_tool_required: bool = False,
    emit_cross_tool: bool | None = None,
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
    if emit_cross_tool is None:
        emit_cross_tool = cross_tool_required
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
    printf '%s\n' '{{"backend_verification":true}}' \
      >target/formal/sumeragi_v2/verus_evidence.json
    printf '%s\n' 'fixture production Verus verification passed' \
      >target/formal/sumeragi_v2/verus.log
    printf '%s\n' \
      '{{"event":"start","memory_limit_bytes":2147483648,"sample_interval_seconds":0.25,"schema_version":1}}' \
      '{{"accounting_method":"rss","event":"sample","memory_bytes":4096,"memory_limit_bytes":2147483648,"physical_footprint_bytes":0,"process_count":1,"rss_bytes":4096,"schema_version":1}}' \
      '{{"event":"summary","exit_reason":"completed","exit_status":0,"memory_limit_bytes":2147483648,"peak_memory_bytes":4096,"sample_interval_seconds":0.25,"schema_version":1}}' \
      >target/formal/sumeragi_v2/tlaps_resource.jsonl
    printf '%s\n' \
      '{{"event":"summary","exit_reason":"completed","exit_status":0,"memory_limit_bytes":2147483648,"peak_memory_bytes":4096,"sample_interval_seconds":0.25,"schema_version":1}}' \
      >target/formal/sumeragi_v2/tlaps_resource_summary.json
    if [[ "${{FORMAL_EMIT_CROSS_TOOL:-0}}" == 1 ]]; then
      printf '%s\n' '{{"backend_verification":true,"canonical":true}}' \
        >target/formal/sumeragi_v2/cross_tool_evidence.json
    fi
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
  *check_sumeragi_v2_proof_ledger.py)
    case " $* " in
      *" --print-cross-tool-obligations "*)
        if [[ "$FORMAL_CROSS_TOOL_REQUIRED" == 1 ]]; then
          printf '%s\n' 'effective-lock-body-acquisition-production-refinement'
        fi
        exit 0
        ;;
    esac
    if [[ "$FORMAL_CROSS_TOOL_REQUIRED" == 1 ]]; then
      [[ " $* " == *" --verus-evidence "* ]] || exit 86
      [[ " $* " == *" --verus-log "* ]] || exit 90
      [[ " $* " == *" --cross-tool-evidence "* ]] || exit 87
    else
      [[ " $* " == *" --verus-evidence "* ]] || exit 88
      [[ " $* " == *" --verus-log "* ]] || exit 90
      [[ " $* " != *" --cross-tool-evidence "* ]] || exit 89
    fi
    exit "$FORMAL_CHECKER_STATUS"
    ;;
  *sumeragi_v2_verus_evidence.py) exit 0 ;;
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
    env["FORMAL_CROSS_TOOL_REQUIRED"] = "1" if cross_tool_required else "0"
    env["FORMAL_EMIT_CROSS_TOOL"] = "1" if emit_cross_tool else "0"
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
    assert fields["verus_evidence_sha256"] == _sha256(
        invocation / "verus_evidence.json"
    )
    assert fields["verus_log_sha256"] == _sha256(invocation / "verus.log")
    assert "cross_tool_evidence_sha256" not in fields
    assert not (invocation / "cross_tool_evidence.json").exists()
    assert fields["harness_cargo_lock_sha256"] == _sha256(
        invocation / "harness-Cargo.lock"
    )
    assert fields["formal_toolchain_sha256"] == _sha256(
        invocation / "formal-toolchain.tsv"
    )
    assert fields["tlaps_resource_jsonl_sha256"] == _sha256(
        invocation / "tlaps_resource.jsonl"
    )
    assert fields["tlaps_resource_summary_sha256"] == _sha256(
        invocation / "tlaps_resource_summary.json"
    )
    assert _fields(invocation / "formal-toolchain.tsv")["tlaps_threads"] == "1"
    assert (invocation / "formal-gate.log").read_text(encoding="utf-8").splitlines()[
        -1
    ] == FINAL_MARKER
    assert pointer.read_text(encoding="utf-8").strip() == str(completion)
    assert not (evidence / ".formal-release.lock").exists()


def test_formal_launcher_archives_required_cross_tool_evidence(
    tmp_path: Path,
) -> None:
    launcher, env, evidence = _fixture(tmp_path, cross_tool_required=True)

    result = _run(launcher, env)

    assert result.returncode == 0, result.stderr
    invocation = _invocation(evidence)
    cross_tool = invocation / "cross_tool_evidence.json"
    assert cross_tool.is_file()
    assert _fields(invocation / "COMPLETED.tsv")[
        "cross_tool_evidence_sha256"
    ] == _sha256(cross_tool)


def test_formal_launcher_rejects_missing_required_cross_tool_evidence(
    tmp_path: Path,
) -> None:
    launcher, env, evidence = _fixture(
        tmp_path, cross_tool_required=True, emit_cross_tool=False
    )

    result = _run(launcher, env)

    assert result.returncode == 1
    assert "did not produce required cross-tool evidence" in result.stderr
    assert not (_invocation(evidence) / "COMPLETED.tsv").exists()


def test_formal_launcher_rejects_cross_tool_evidence_while_dormant(
    tmp_path: Path,
) -> None:
    launcher, env, evidence = _fixture(tmp_path, emit_cross_tool=True)

    result = _run(launcher, env)

    assert result.returncode == 1
    assert "produced forbidden dormant cross-tool evidence" in result.stderr
    assert not (_invocation(evidence) / "COMPLETED.tsv").exists()


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
        ROOT_DIR
        / "scripts"
        / "formal"
        / "run_sumeragi_v2_decision_recovery_lifecycle_mutation.sh",
        ROOT_DIR
        / "scripts"
        / "formal"
        / "run_sumeragi_v2_ingress_causal_freshness_mutation.sh",
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


def test_ingress_causal_freshness_mutation_is_release_gated_and_pinned() -> None:
    relative_runner = (
        "scripts/formal/run_sumeragi_v2_ingress_causal_freshness_mutation.sh"
    )
    gate = (ROOT_DIR / "ci" / "check_sumeragi_formal.sh").read_text(
        encoding="utf-8"
    )
    assert gate.count(f"bash {relative_runner}") == 1
    assert gate.index(relative_runner) < gate.index("run_sumeragi_v2_tlc.sh")

    runner = (ROOT_DIR / relative_runner).read_text(encoding="utf-8")
    assert 'readonly TLA2TOOLS_VERSION="1.7.4"' in runner
    assert (
        'readonly TLA2TOOLS_SHA256="936a262061c914694dfd669a543be24573'
        'c45d5aa0ff20a8b96b23d01e050e88"'
        in runner
    )
    assert 'readonly EXPECTED_JAVA_VERSION=\'openjdk version "21.0.11"\'' in runner
    assert 'readonly MODEL="SumeragiV2IngressCausalFreshnessMutation.tla"' in runner
    assert "resolve_java.sh" in runner
    assert '"$JAVA_BIN"' in runner
    assert "-fp 96 -seed 139154308881391968" in runner

    fixed_case = (
        "run_case scheduler-wide-fixed ingress_causal_freshness_fixed.cfg 0 \\\n"
        '  "Model checking completed. No error has been found." \\\n'
        '  "2 states generated, 2 distinct states found, 0 states left on queue." '
        '\\\n  "depth of the complete state graph search is 2"'
    )
    mutation_case = (
        "run_case inflight-only-mutation \\\n"
        "  ingress_causal_freshness_inflight_only_bug.cfg 12 \\\n"
        '  "Invariant PairwiseSingleOwnership is violated." \\\n'
        "  'phase = \"Admitted\"' \\\n"
        '  "trackedDuplicateCreated = TRUE" \\\n'
        '  "queuedDuplicateCreated = TRUE" \\\n'
        '  "2 states generated, 2 distinct states found, 0 states left on queue." '
        '\\\n  "depth of the complete state graph search is 2"'
    )
    assert fixed_case in runner
    assert mutation_case in runner
    assert runner.count("depth of the complete state graph search is 2") == 2
    assert runner.count(
        "2 states generated, 2 distinct states found, 0 states left on queue."
    ) == 2

    formal_dir = ROOT_DIR / "docs" / "formal" / "sumeragi_v2"
    fixed = (formal_dir / "ingress_causal_freshness_fixed.cfg").read_text(
        encoding="utf-8"
    )
    mutation = (
        formal_dir / "ingress_causal_freshness_inflight_only_bug.cfg"
    ).read_text(encoding="utf-8")
    assert fixed.splitlines().count(
        "CONSTANT RequireSchedulerWideFreshness = TRUE"
    ) == 1
    assert "INVARIANT SchedulerWideDuplicateCoalesced\n" in fixed
    assert "INVARIANT IngressOccurrenceConsumedExactlyOnce\n" in fixed
    assert mutation.splitlines().count(
        "CONSTANT RequireSchedulerWideFreshness = FALSE"
    ) == 1
    assert "INVARIANT PairwiseSingleOwnership\n" in mutation

    model = (
        formal_dir / "SumeragiV2IngressCausalFreshnessMutation.tla"
    ).read_text(encoding="utf-8")
    assert model.splitlines().count("CONSTANT RequireSchedulerWideFreshness") == 1
    assert (
        "IF RequireSchedulerWideFreshness\n  THEN ~CandidateScheduled(candidate)"
        in model
    )
    assert "ELSE ~CandidateInFlight(candidate)" in model
