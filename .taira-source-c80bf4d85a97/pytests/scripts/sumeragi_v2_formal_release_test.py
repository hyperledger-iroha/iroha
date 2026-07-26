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
      $'schema_version\t1' \
      $'backend\tapalache' \
      $'result_count\t3' \
      >target/formal/sumeragi_v2/multilane_apalache_evidence.tsv
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
    assert fields["multilane_apalache_evidence_sha256"] == _sha256(
        invocation / "multilane_apalache_evidence.tsv"
    )
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
        / "run_sumeragi_v2_restart_locked_fetch_order_mutation.sh",
        ROOT_DIR
        / "scripts"
        / "formal"
        / "run_sumeragi_v2_persist_install_generation_mutation.sh",
        ROOT_DIR
        / "scripts"
        / "formal"
        / "run_sumeragi_v2_persist_install_validation_mutation.sh",
        ROOT_DIR
        / "scripts"
        / "formal"
        / "run_sumeragi_v2_replay_locked_body_carrier_mutation.sh",
        ROOT_DIR
        / "scripts"
        / "formal"
        / "run_sumeragi_v2_certificate_ref_recovery_mutation.sh",
        ROOT_DIR
        / "scripts"
        / "formal"
        / "run_sumeragi_v2_certified_response_source_lineage_mutation.sh",
        ROOT_DIR
        / "scripts"
        / "formal"
        / "run_sumeragi_v2_certified_response_identity_separation_mutation.sh",
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


def test_restart_locked_fetch_order_mutation_is_release_gated_and_pinned() -> None:
    relative_runner = (
        "scripts/formal/run_sumeragi_v2_restart_locked_fetch_order_mutation.sh"
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
    assert (
        'readonly MODEL="SumeragiV2RestartLockedFetchOrderMutation.tla"' in runner
    )
    assert (
        'readonly SANY_SUCCESS_MARKER="Semantic processing of module '
        'SumeragiV2RestartLockedFetchOrderMutation"'
        in runner
    )
    assert (
        "sany_last_nonblank=\"$(awk 'NF { line = $0 } END { print line }' "
        '\"${run_dir}/sany.log\")\"'
        in runner
    )
    assert '[[ "$sany_last_nonblank" == "$SANY_SUCCESS_MARKER" ]]' in runner
    assert "-fp 96 -seed 139154308881391968" in runner
    assert "resolve_java.sh" in runner
    assert '"$JAVA_BIN"' in runner
    assert (
        "run_case repaired \\\n"
        "  restart_locked_fetch_order_fixed.cfg 0 \\\n"
        '  "TLC2 Version 2.19" \\\n'
        '  "Model checking completed. No error has been found."'
        in runner
    )
    assert (
        "run_case dropped-fetch \\\n"
        "  restart_locked_fetch_order_drop_fetch_bug.cfg 12 \\\n"
        '  "Invariant RestartReplayHasExactOwnership is violated."'
        in runner
    )
    assert (
        "run_case reversed-order \\\n"
        "  restart_locked_fetch_order_reversed_bug.cfg 12 \\\n"
        '  "Invariant LockedFetchPrecedesFirstSign is violated."'
        in runner
    )

    formal_dir = ROOT_DIR / "docs" / "formal" / "sumeragi_v2"
    model = (formal_dir / "SumeragiV2RestartLockedFetchOrderMutation.tla").read_text(
        encoding="utf-8"
    )
    assert (
        'Mode = "Repaired" ->\n'
        "              <<LockedFetchRequest, DurableSignRequest>>"
        in model
    )
    assert "Len(replay) = 2" in model
    assert "fetchIndex < signIndex" in model
    fixed = (formal_dir / "restart_locked_fetch_order_fixed.cfg").read_text(
        encoding="utf-8"
    )
    dropped = (
        formal_dir / "restart_locked_fetch_order_drop_fetch_bug.cfg"
    ).read_text(encoding="utf-8")
    reversed_order = (
        formal_dir / "restart_locked_fetch_order_reversed_bug.cfg"
    ).read_text(encoding="utf-8")
    assert fixed.count('CONSTANT Mode = "Repaired"') == 1
    assert "INVARIANT RestartReplayHasExactOwnership\n" in fixed
    assert "INVARIANT LockedFetchPrecedesFirstSign\n" in fixed
    assert dropped.count('CONSTANT Mode = "DropFetch"') == 1
    assert "INVARIANT RestartReplayHasExactOwnership\n" in dropped
    assert reversed_order.count('CONSTANT Mode = "Reverse"') == 1
    assert "INVARIANT LockedFetchPrecedesFirstSign\n" in reversed_order


def test_persist_install_generation_mutation_is_release_gated_and_pinned() -> None:
    relative_runner = (
        "scripts/formal/run_sumeragi_v2_persist_install_generation_mutation.sh"
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
    assert (
        'readonly MODEL="SumeragiV2PersistInstallGenerationMutation.tla"' in runner
    )
    assert (
        'readonly SANY_SUCCESS_MARKER="Semantic processing of module '
        'SumeragiV2PersistInstallGenerationMutation"'
        in runner
    )
    assert (
        "sany_last_nonblank=\"$(awk 'NF { line = $0 } END { print line }' "
        '\"${run_dir}/sany.log\")\"'
        in runner
    )
    assert '[[ "$sany_last_nonblank" == "$SANY_SUCCESS_MARKER" ]]' in runner
    assert "-fp 96 -seed 139154308881391968" in runner
    assert "resolve_java.sh" in runner
    assert '"$JAVA_BIN"' in runner
    assert (
        "run_case repaired-overflow-rejection \\\n"
        "  persist_install_generation_fixed.cfg 0 \\\n"
        '  "TLC2 Version 2.19" \\\n'
        '  "Model checking completed. No error has been found."'
        in runner
    )
    assert (
        "run_case saturating-partial-commit \\\n"
        "  persist_install_generation_saturation_bug.cfg 12 \\\n"
        '  "Invariant OverflowRejectionPreservesCompleteState is violated."'
        in runner
    )

    formal_dir = ROOT_DIR / "docs" / "formal" / "sumeragi_v2"
    model = (
        formal_dir / "SumeragiV2PersistInstallGenerationMutation.tla"
    ).read_text(encoding="utf-8")
    assert (
        "RejectGenerationOverflow ==\n"
        "  /\\ pendingInstall\n"
        '  /\\ outcome = "Pending"\n'
        "  /\\ generation = MaxGeneration\n"
        '  /\\ outcome\' = "OverflowRejected"\n'
        "  /\\ UNCHANGED <<generation, durableSnapshot, pendingInstall>>"
        in model
    )
    assert "durableSnapshot' = PartialSnapshot" in model
    assert "pendingInstall' = FALSE" in model
    fixed = (formal_dir / "persist_install_generation_fixed.cfg").read_text(
        encoding="utf-8"
    )
    mutant = (
        formal_dir / "persist_install_generation_saturation_bug.cfg"
    ).read_text(encoding="utf-8")
    assert fixed.count('Mode = "Repaired"') == 1
    assert fixed.count("MaxGeneration = 2") == 1
    assert "INVARIANT OverflowRejectionPreservesCompleteState\n" in fixed
    assert "INVARIANT InstallSnapshotIsAtomic\n" in fixed
    assert mutant.count('Mode = "SaturatingPartialCommit"') == 1
    assert "INVARIANT OverflowRejectionPreservesCompleteState\n" in mutant
    assert "INVARIANT InstallSnapshotIsAtomic\n" in mutant


def test_persist_install_validation_mutation_is_release_gated_and_pinned() -> None:
    relative_runner = (
        "scripts/formal/run_sumeragi_v2_persist_install_validation_mutation.sh"
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
    assert (
        'readonly MODEL="SumeragiV2PersistInstallValidationMutation.tla"'
        in runner
    )
    assert (
        'readonly SANY_SUCCESS_MARKER="Semantic processing of module '
        'SumeragiV2PersistInstallValidationMutation"'
        in runner
    )
    assert (
        "sany_last_nonblank=\"$(awk 'NF { line = $0 } END { print line }' "
        '\"${run_dir}/sany.log\")\"'
        in runner
    )
    assert '[[ "$sany_last_nonblank" == "$SANY_SUCCESS_MARKER" ]]' in runner
    assert "-fp 97 -seed 712381923" in runner
    assert "resolve_java.sh" in runner
    assert '"$JAVA_BIN"' in runner
    assert (
        "run_case repaired-validation-clear \\\n"
        "  persist_install_validation_fixed.cfg 0 \\\n"
        '  "TLC2 Version 2.19" \\\n'
        '  "Model checking completed. No error has been found."'
        in runner
    )
    assert (
        "run_case retained-stale-validation \\\n"
        "  persist_install_validation_retained_bug.cfg 12 \\\n"
        '  "Invariant NoOrphanedValidationReceipt is violated."'
        in runner
    )

    formal_dir = ROOT_DIR / "docs" / "formal" / "sumeragi_v2"
    model = (
        formal_dir / "SumeragiV2PersistInstallValidationMutation.tla"
    ).read_text(encoding="utf-8")
    assert (
        "validatedBodies' =\n"
        "       {validation \\in validatedBodies:\n"
        '          validation.node # "installing"}'
        in model
    )
    assert "validatedBodies' = validatedBodies" in model
    assert (
        "validation.generation = generation[validation.node]"
        in model
    )
    fixed = (formal_dir / "persist_install_validation_fixed.cfg").read_text(
        encoding="utf-8"
    )
    mutant = (
        formal_dir / "persist_install_validation_retained_bug.cfg"
    ).read_text(encoding="utf-8")
    assert fixed.count('CONSTANT Mode = "Repaired"') == 1
    assert "INVARIANT NoOrphanedValidationReceipt\n" in fixed
    assert "PROPERTY InstallCompletes\n" in fixed
    assert mutant.count('CONSTANT Mode = "RetainStaleValidation"') == 1
    assert "INVARIANT NoOrphanedValidationReceipt\n" in mutant


def test_replay_locked_body_carrier_mutation_is_release_gated_and_pinned() -> None:
    relative_runner = (
        "scripts/formal/run_sumeragi_v2_replay_locked_body_carrier_mutation.sh"
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
    assert (
        'readonly MODEL="SumeragiV2ReplayLockedBodyCarrierMutation.tla"'
        in runner
    )
    assert (
        'readonly SANY_SUCCESS_MARKER="Semantic processing of module '
        'SumeragiV2ReplayLockedBodyCarrierMutation"'
        in runner
    )
    assert "-fp 96 -seed 139154308881391968" in runner
    assert "resolve_java.sh" in runner
    assert '"$JAVA_BIN"' in runner
    assert (
        "run_case fixed \\\n"
        "  replay_locked_body_carrier_fixed.cfg 0 \\\n"
        '  "TLC2 Version 2.19" \\\n'
        '  "Model checking completed. No error has been found."'
        in runner
    )
    assert (
        "run_case dropped-locked-fetch \\\n"
        "  replay_locked_body_carrier_drop_fetch_bug.cfg 12 \\\n"
        '  "Invariant HistoricalLockedBodyRecoveryStageInvariant is violated."'
        in runner
    )
    assert '"State 4: <FinishResponsiveReplay"' in runner

    formal_dir = ROOT_DIR / "docs" / "formal" / "sumeragi_v2"
    model = (
        formal_dir / "SumeragiV2ReplayLockedBodyCarrierMutation.tla"
    ).read_text(encoding="utf-8")
    assert (
        'IF Mode = "Fixed"\n'
        "  THEN <<LockedFetchCandidate, ReplaySignatureCandidate>>\n"
        "  ELSE <<ReplaySignatureCandidate>>"
        in model
    )
    assert "HistoricalLockedBodyNonAuthorityCarrier ==" in model
    assert "FinishResponsiveReplay ==" in model
    fixed = (formal_dir / "replay_locked_body_carrier_fixed.cfg").read_text(
        encoding="utf-8"
    )
    mutant = (
        formal_dir / "replay_locked_body_carrier_drop_fetch_bug.cfg"
    ).read_text(encoding="utf-8")
    assert fixed.count('CONSTANT Mode = "Fixed"') == 1
    assert "INVARIANT ReplayLockedBodyCarrierInvariant\n" in fixed
    assert "INVARIANT FinishDoesNotDropLastLockedBodyOwner\n" in fixed
    assert mutant.count('CONSTANT Mode = "DropLockedFetch"') == 1
    assert "INVARIANT HistoricalLockedBodyRecoveryStageInvariant\n" in mutant
    assert "INVARIANT ReplayLockedBodyCarrierInvariant\n" not in mutant


def test_certificate_ref_recovery_mutation_is_release_gated_and_pinned() -> None:
    relative_runner = (
        "scripts/formal/run_sumeragi_v2_certificate_ref_recovery_mutation.sh"
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
    assert (
        'readonly MODEL="SumeragiV2CertificateRefRecoveryMutation.tla"'
        in runner
    )
    assert (
        'readonly SANY_SUCCESS_MARKER="Semantic processing of module '
        'SumeragiV2CertificateRefRecoveryMutation"'
        in runner
    )
    assert "-fp 96 -seed 139154308881391968" in runner
    assert "resolve_java.sh" in runner
    assert '"$JAVA_BIN"' in runner
    assert (
        "run_case fixed-full-certificate-ref \\\n"
        "  certificate_ref_recovery_fixed.cfg 0 \\\n"
        '  "TLC2 Version 2.19" \\\n'
        '  "Model checking completed. No error has been found."'
        in runner
    )
    assert (
        "run_case exact-qc-equality-mutant \\\n"
        "  certificate_ref_recovery_exact_qc_bug.cfg 12 \\\n"
        '  "TLC2 Version 2.19" \\\n'
        '  "Invariant HistoricalLockedCommitRecoveryProgress is violated."'
        in runner
    )

    formal_dir = ROOT_DIR / "docs" / "formal" / "sumeragi_v2"
    model = (
        formal_dir / "SumeragiV2CertificateRefRecoveryMutation.tla"
    ).read_text(encoding="utf-8")
    for stable_field in (
        "left.context = right.context",
        "left.height = right.height",
        "left.view = right.view",
        "left.phase = right.phase",
        "left.subject = right.subject",
    ):
        assert stable_field in model
    assert "PrepareQcA # PrepareQcB" in model
    assert "request.qc = qc" in model
    fixed = (formal_dir / "certificate_ref_recovery_fixed.cfg").read_text(
        encoding="utf-8"
    )
    mutant = (
        formal_dir / "certificate_ref_recovery_exact_qc_bug.cfg"
    ).read_text(encoding="utf-8")
    assert fixed.count('CONSTANT Mode = "FullCertificateRef"') == 1
    assert "INVARIANT StableReferenceFieldsCannotAlias\n" in fixed
    assert "INVARIANT HistoricalLockedCommitRecoveryProgress\n" in fixed
    assert mutant.count('CONSTANT Mode = "ExactQcEquality"') == 1
    assert "INVARIANT HistoricalLockedCommitRecoveryProgress\n" in mutant


def test_certified_response_source_lineage_mutation_is_release_gated_and_pinned() -> None:
    relative_runner = (
        "scripts/formal/"
        "run_sumeragi_v2_certified_response_source_lineage_mutation.sh"
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
    assert (
        'readonly MODEL="SumeragiV2CertifiedResponseSourceLineageMutation.tla"'
        in runner
    )
    assert (
        'readonly SANY_SUCCESS_MARKER="Semantic processing of module '
        'SumeragiV2CertifiedResponseSourceLineageMutation"'
        in runner
    )
    assert "-fp 96 -seed 139154308881391968" in runner
    assert "resolve_java.sh" in runner
    assert '"$JAVA_BIN"' in runner
    assert (
        "run_case embedded-cited-signer-surrogate-fixed \\\n"
        "  certified_response_source_lineage_fixed.cfg 0 \\\n"
        '  "TLC2 Version 2.19" \\\n'
        '  "Model checking completed. No error has been found."'
        in runner
    )
    assert (
        "run_case outer-transport-source-mutant \\\n"
        "  certified_response_source_lineage_outer_source_bug.cfg 12 \\\n"
        '  "TLC2 Version 2.19" \\\n'
        '  "Invariant DecisionRecoveryOwnerRetained is violated."'
        in runner
    )

    formal_dir = ROOT_DIR / "docs" / "formal" / "sumeragi_v2"
    model = (
        formal_dir / "SumeragiV2CertifiedResponseSourceLineageMutation.tla"
    ).read_text(encoding="utf-8")
    assert (
        "CertifiedResponse.envelope.citedResponder \\in CommitQc.signers"
        in model
    )
    assert "CertifiedResponse.source \\in CommitQc.signers" in model
    assert (
        'IF Mode = "EmbeddedCitedSignerSurrogate"\n'
        "  THEN ExplicitCitedResponderOwnsResponse\n"
        "  ELSE OuterTransportSourceOwnsResponse"
        in model
    )
    fixed = (
        formal_dir / "certified_response_source_lineage_fixed.cfg"
    ).read_text(encoding="utf-8")
    mutant = (
        formal_dir / "certified_response_source_lineage_outer_source_bug.cfg"
    ).read_text(encoding="utf-8")
    assert fixed.count('CONSTANT Mode = "EmbeddedCitedSignerSurrogate"') == 1
    assert "INVARIANT HonestCertifiedResponseShape\n" in fixed
    assert "INVARIANT DecisionRecoveryOwnerRetained\n" in fixed
    assert mutant.count('CONSTANT Mode = "OuterTransportSource"') == 1
    assert "INVARIANT DecisionRecoveryOwnerRetained\n" in mutant


def test_certified_response_identity_separation_mutation_is_release_gated_and_pinned() -> None:
    relative_runner = (
        "scripts/formal/"
        "run_sumeragi_v2_certified_response_identity_separation_mutation.sh"
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
    assert (
        'readonly MODEL="SumeragiV2CertifiedResponseIdentitySeparationMutation.tla"'
        in runner
    )
    assert (
        'readonly SANY_SUCCESS_MARKER="Semantic processing of module '
        'SumeragiV2CertifiedResponseIdentitySeparationMutation"'
        in runner
    )
    assert "-fp 97 -seed 139154308881391968" in runner
    assert "resolve_java.sh" in runner
    assert '"$JAVA_BIN"' in runner
    assert (
        "run_case separated-identities-fixed \\\n"
        "  certified_response_identity_separation_fixed.cfg 0 \\\n"
        '  "TLC2 Version 2.19" \\\n'
        '  "Model checking completed. No error has been found."'
        in runner
    )
    assert (
        "run_case archive-server-as-qc-signer-mutant \\\n"
        "  certified_response_identity_archive_signer_bug.cfg 12 \\\n"
        '  "TLC2 Version 2.19" \\\n'
        '  "Invariant ValidRotatedArchiveResponseAccepted is violated."'
        in runner
    )
    assert (
        "run_case archive-server-as-route-target-mutant \\\n"
        "  certified_response_identity_route_target_bug.cfg 12 \\\n"
        '  "TLC2 Version 2.19" \\\n'
        '  "Invariant ValidRotatedArchiveResponseAccepted is violated."'
        in runner
    )
    assert (
        "route-target authority conflation rejected a valid non-target "
        "recovery response"
        in runner
    )
    assert runner.count('"State 2: <ProcessResponse"') == 2
    assert runner.count('\'/\\ lastAttempt = "Honest"\'') == 2

    formal_dir = ROOT_DIR / "docs" / "formal" / "sumeragi_v2"
    model = (
        formal_dir
        / "SumeragiV2CertifiedResponseIdentitySeparationMutation.tla"
    ).read_text(encoding="utf-8")
    for field in (
        "via |-> UntrustedRelay",
        "archiveServer |-> RotatedArchive",
        "requestHash |-> ExactRequestHash",
        "citedResponder |-> FrozenSigner",
    ):
        assert field in model
    assert "routeTarget |-> OriginalRouteTarget" in model
    assert "OriginalRouteTarget \\in CurrentVotingRoster" in model
    assert "RotatedArchive \\notin CurrentVotingRoster" in model
    assert "CurrentVotingPower(RotatedArchive) = 0" in model
    assert "response.signatureOwner = response.archiveServer" in model
    assert (
        "response.citedResponder \\in CertifiedRequest.certificate.signers"
        in model
    )
    assert "FrozenSigner \\in CommitQc.signers \\ {Requester}" in model
    assert "response.archiveServer \\in CommitQc.signers" in model
    assert (
        "WrongRequestPreimage ==\n"
        '  [ExactRequestPreimage EXCEPT !.subject = "different-decided-block-12"]'
        in model
    )
    assert (
        "WrongRequestHash ==\n"
        "  [exactSignedRequest |->\n"
        "    [preimage |-> WrongRequestPreimage,\n"
        "     signature |-> WrongRequestSignature]]"
        in model
    )
    assert '"different-exact-signed-request"' not in model
    separated_authorization = model.split(
        "SeparatedResponseAuthorized(response) ==", 1
    )[1].split("ConflatedResponseAuthorized(response) ==", 1)[0]
    assert "routeTarget" not in separated_authorization
    assert (
        "RouteBoundResponseAuthorized(response) ==\n"
        "  /\\ SeparatedResponseAuthorized(response)\n"
        "  /\\ response.archiveServer = CertifiedRequest.routeTarget"
        in model
    )
    for negative in (
        "RequestHashMismatchResponse",
        "CoordinateMismatchResponse",
        "CitedSignerMismatchResponse",
        "RelaySignedResponse",
    ):
        assert f"~SeparatedResponseAuthorized({negative})" in model
    assert (
        "responseRejected =>\n"
        "    /\\ outstandingRequest = CertifiedRequest\n"
        "    /\\ requestLive\n"
        "    /\\ ~candidateScheduled"
        in model
    )
    fixed = (
        formal_dir / "certified_response_identity_separation_fixed.cfg"
    ).read_text(encoding="utf-8")
    archive_signer_mutant = (
        formal_dir / "certified_response_identity_archive_signer_bug.cfg"
    ).read_text(encoding="utf-8")
    route_target_mutant = (
        formal_dir / "certified_response_identity_route_target_bug.cfg"
    ).read_text(encoding="utf-8")
    assert fixed.count('CONSTANT Mode = "SeparatedIdentities"') == 1
    assert "INVARIANT ExactNegativeControlsRejected\n" in fixed
    assert "INVARIANT RejectedResponseRetainsExactRequest\n" in fixed
    assert "INVARIANT ValidRotatedArchiveResponseAccepted\n" in fixed
    assert (
        archive_signer_mutant.count(
            'CONSTANT Mode = "ArchiveServerMustBeQcSigner"'
        )
        == 1
    )
    assert (
        "INVARIANT ExactRequestRecoveryOwnerRetained\n"
        in archive_signer_mutant
    )
    assert (
        "INVARIANT ValidRotatedArchiveResponseAccepted\n"
        in archive_signer_mutant
    )
    assert (
        route_target_mutant.count(
            'CONSTANT Mode = "ArchiveServerMustMatchRouteTarget"'
        )
        == 1
    )
    assert "INVARIANT ExactNegativeControlsRejected\n" in route_target_mutant
    assert (
        "INVARIANT RejectedResponseRetainsExactRequest\n"
        in route_target_mutant
    )
    assert (
        "INVARIANT ValidRotatedArchiveResponseAccepted\n"
        in route_target_mutant
    )


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
    assert 'readonly EXPECTED_JAVA_VERSION=\'openjdk version "21.0.12"\'' in runner
    assert runner.count(
        'readonly SANY_SUCCESS_MARKER="Semantic processing of module '
        'SumeragiV2IngressCausalFreshnessMutation"'
    ) == 1
    assert runner.count(
        "sany_last_nonblank=\"$(awk 'NF { line = $0 } END { print line }' "
        '\"${run_dir}/sany.log\")\"'
    ) == 1
    assert runner.count(
        '[[ "$sany_last_nonblank" == "$SANY_SUCCESS_MARKER" ]]'
    ) == 1
    assert 'grep -Fq "$SANY_SUCCESS_MARKER" "${run_dir}/sany.log"' not in runner
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
