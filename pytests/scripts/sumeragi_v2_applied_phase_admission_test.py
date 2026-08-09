"""Applied-phase admission and formal-gate ordering tests."""

from __future__ import annotations

import importlib.util
import shutil
import sys
from pathlib import Path

import pytest


ROOT_DIR = Path(__file__).resolve().parents[2]
SCRIPT = ROOT_DIR / "scripts" / "formal" / "check_sumeragi_v2_proof_ledger.py"


def load_checker():
    """Load the Sumeragi v2 proof-ledger checker under a stable module name."""

    spec = importlib.util.spec_from_file_location("sumeragi_v2_proof_ledger", SCRIPT)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def copy_applied_phase_admission_mutation_fixture(
    tmp_path: Path, module
) -> tuple[Path, Path]:
    """Copy the exact eight-file model corpus and production admission seam."""

    repo_root = tmp_path / "repo"
    formal_dir = repo_root / "formal" / "sumeragi_v2"
    formal_dir.mkdir(parents=True)
    for name in module.APPLIED_PHASE_ADMISSION_MUTATION_FORMAL_ARTIFACTS:
        shutil.copy2(module.FORMAL_DIR / name, formal_dir / name)
    runner = repo_root / module.APPLIED_PHASE_ADMISSION_MUTATION_RUNNER
    runner.parent.mkdir(parents=True)
    shutil.copy2(
        ROOT_DIR / module.APPLIED_PHASE_ADMISSION_MUTATION_RUNNER,
        runner,
    )
    ci = repo_root / "ci" / "check_sumeragi_formal.sh"
    ci.parent.mkdir(parents=True)
    shutil.copy2(ROOT_DIR / "ci/check_sumeragi_formal.sh", ci)
    for relative in (
        "crates/iroha_core/src/sumeragi/v2.rs",
        "crates/iroha_core/src/sumeragi/v2_runtime.rs",
    ):
        destination = repo_root / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT_DIR / relative, destination)
    return repo_root, formal_dir


def test_applied_phase_admission_source_seal_covers_exact_corpus_and_production(
    tmp_path: Path,
) -> None:
    module = load_checker()
    repo_root, formal_dir = copy_applied_phase_admission_mutation_fixture(
        tmp_path, module
    )

    assert len(module.APPLIED_PHASE_ADMISSION_MUTATION_FORMAL_ARTIFACTS) == 7
    assert (
        sum(
            name.endswith(".tla")
            for name in module.APPLIED_PHASE_ADMISSION_MUTATION_FORMAL_ARTIFACTS
        )
        == 1
    )
    assert (
        sum(
            name.endswith(".cfg")
            for name in module.APPLIED_PHASE_ADMISSION_MUTATION_FORMAL_ARTIFACTS
        )
        == 6
    )
    assert len(module.APPLIED_PHASE_ADMISSION_MUTATION_SHA256) == 8
    assert (
        module._applied_phase_admission_mutation_source_fidelity_errors(
            formal_dir, repo_root
        )
        == []
    )


def test_applied_phase_admission_runner_rejects_status_and_witness_mutants(
    tmp_path: Path,
) -> None:
    module = load_checker()
    repo_root, _formal_dir = copy_applied_phase_admission_mutation_fixture(
        tmp_path, module
    )
    runner = repo_root / module.APPLIED_PHASE_ADMISSION_MUTATION_RUNNER
    source = runner.read_text(encoding="utf-8")

    runner.write_text(
        source.replace(
            "applied_phase_admission_fixed.cfg 0 \\",
            "applied_phase_admission_fixed.cfg 12 \\",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._applied_phase_admission_mutation_runner_errors(repo_root)
    assert any("found repaired=0, mutants=6" in error for error in errors), errors

    runner.write_text(
        source.replace(
            "Invariant AppliedExactRetryPreservesOrdinal is violated.",
            "Invariant AppliedPhaseHasNoPhysicalOwner is violated.",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._applied_phase_admission_mutation_runner_errors(repo_root)
    assert any(
        "applied-phase role applied_phase_post_apply_ordinal_bug.cfg" in error
        for error in errors
    ), errors

    runner.write_text(
        source.replace(
            "malformed-plus-stale callbacks reject before well-formed stale "
            "callbacks coalesce marker-free",
            "stale callbacks coalesce",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._applied_phase_admission_mutation_runner_errors(repo_root)
    assert any(
        "malformed/stale ordering summary" in error for error in errors
    ), errors


@pytest.mark.parametrize(
    "artifact_name",
    (
        "SumeragiV2AppliedPhaseAdmissionMutation.tla",
        "applied_phase_post_apply_ordinal_bug.cfg",
        "applied_phase_malformed_callback_stale_tag_hidden_bug.cfg",
        "scripts/formal/run_sumeragi_v2_applied_phase_admission_mutations.sh",
    ),
)
def test_applied_phase_admission_source_seal_rejects_stale_artifact(
    tmp_path: Path,
    artifact_name: str,
) -> None:
    module = load_checker()
    repo_root, formal_dir = copy_applied_phase_admission_mutation_fixture(
        tmp_path, module
    )
    path = (
        repo_root / artifact_name
        if "/" in artifact_name
        else formal_dir / artifact_name
    )
    path.write_text(
        path.read_text(encoding="utf-8") + "\n\\* stale mutation\n",
        encoding="utf-8",
    )

    errors = module._applied_phase_admission_mutation_source_fidelity_errors(
        formal_dir, repo_root
    )

    assert any(
        str(path) in error and "must match exact reviewed SHA-256" in error
        for error in errors
    ), errors


def test_applied_phase_production_preflight_must_precede_ordinal_allocation(
    tmp_path: Path,
) -> None:
    module = load_checker()
    repo_root, _formal_dir = copy_applied_phase_admission_mutation_fixture(
        tmp_path, module
    )
    runtime_path = (
        repo_root / "crates/iroha_core/src/sumeragi/v2_runtime.rs"
    )
    source = runtime_path.read_text(encoding="utf-8")
    runtime_context = (
        (
            "impl",
            "<",
            "D",
            ":",
            "RuntimeDriver",
            ">",
            "SerializedV2Runtime",
            "<",
            "D",
            ">",
        ),
    )
    enqueue = next(
        item
        for item in module.rust_items(source, "enqueue")
        if item.brace_context == runtime_context
    )
    old = "let preflight = self.command_admission_preflight(tag, class, &command)?;"
    assert enqueue.source.count(old) == 1
    item_start = source.index(enqueue.source)
    item_end = item_start + len(enqueue.source)
    runtime_path.write_text(
        source[:item_start]
        + enqueue.source.replace(
            old,
            "let preflight = RuntimeCommandAdmissionPreflight::Admit;",
            1,
        )
        + source[item_end:],
        encoding="utf-8",
    )

    errors = (
        module._applied_phase_admission_production_source_fidelity_errors(
            repo_root
        )
    )

    assert any(
        "preflight must coalesce or restore the exact dormant owner before "
        "physical enqueue and fresh ordinal allocation"
        in error
        for error in errors
    ), errors


def test_applied_phase_busy_scope_requires_both_evidence_phases(
    tmp_path: Path,
) -> None:
    module = load_checker()
    repo_root, _formal_dir = copy_applied_phase_admission_mutation_fixture(
        tmp_path, module
    )
    runtime_path = repo_root / "crates/iroha_core/src/sumeragi/v2_runtime.rs"
    source = runtime_path.read_text(encoding="utf-8")
    item = module.rust_items(
        source,
        "completion_retries_coalesce_across_ingress_and_busy_deferred_ownership",
    )[0]
    old = "DeferredBodyPipelineStageForTest::ValidationSucceeded,"
    assert item.source.count(old) == 1
    item_start = source.index(item.source)
    item_end = item_start + len(item.source)
    runtime_path.write_text(
        source[:item_start]
        + item.source.replace(
            old,
            "DeferredBodyPipelineStageForTest::BodyStored,",
            1,
        )
        + source[item_end:],
        encoding="utf-8",
    )

    errors = (
        module._applied_phase_admission_production_source_fidelity_errors(
            repo_root
        )
    )

    assert any(
        "ValidationSucceeded Busy-owner witness" in error for error in errors
    ), errors


def test_formal_gate_validates_fresh_evidence_before_tlc_and_replay() -> None:
    """Pin the reviewed ordering of structural, mutation, and backend gates."""

    source = (ROOT_DIR / "ci" / "check_sumeragi_formal.sh").read_text()

    structural = source.index("check_sumeragi_v2_proof_ledger.py")
    tlaps = source.index("run_sumeragi_v2_tlaps.sh")
    release = source.index("--release")
    mutations = source.index("run_sumeragi_v2_service_rank_mutation.sh")
    productive_mutations = source.index("run_sumeragi_v2_productive_mutation.sh")
    candidate_restart = source.index(
        "run_sumeragi_v2_candidate_restart_mutation.sh"
    )
    progress_mutations = source.index("run_sumeragi_v2_progress_mutations.sh")
    effect_capacity_mutations = source.index(
        "run_sumeragi_v2_effect_capacity_ownership_mutation.sh"
    )
    applied_phase_admission_mutations = source.index(
        "run_sumeragi_v2_applied_phase_admission_mutations.sh"
    )
    liveness_ownership_mutations = source.index(
        "run_sumeragi_v2_liveness_ownership_mutations.sh"
    )
    indexed_service_activation_mutations = source.index(
        "run_sumeragi_v2_indexed_service_activation_mutations.sh"
    )
    adequate_leader_readiness_mutations = source.index(
        "run_sumeragi_v2_adequate_leader_readiness_mutations.sh"
    )
    indexed_height_mutation = source.index(
        "run_sumeragi_v2_indexed_height_mutation.sh"
    )
    item_carrier_typing_mutation = source.index(
        "run_sumeragi_v2_item_carrier_typing_mutation.sh"
    )
    reply_writer_deadline_mutations = source.index(
        "run_sumeragi_v2_reply_writer_deadline_mutations.sh"
    )
    historical_discovery_mutation = source.index(
        "run_sumeragi_v2_historical_discovery_occurrence_rank_mutation.sh"
    )
    typed_rollover_mutations = source.index(
        "run_sumeragi_v2_typed_rollover_handoff_mutations.sh"
    )
    tlc = source.index("run_sumeragi_v2_tlc.sh")
    replay = source.index("check_sumeragi_v2_replay_trace.sh")
    verus = source.index("verify_sumeragi_v2.sh")
    final_release = source.rindex("--release")
    final_marker = source.index("Sumeragi v2 formal gate passed")
    assert (
        structural
        < tlaps
        < release
        < mutations
        < productive_mutations
        < candidate_restart
        < progress_mutations
        < effect_capacity_mutations
        < applied_phase_admission_mutations
        < liveness_ownership_mutations
        < indexed_service_activation_mutations
        < adequate_leader_readiness_mutations
        < indexed_height_mutation
        < item_carrier_typing_mutation
        < reply_writer_deadline_mutations
        < historical_discovery_mutation
        < typed_rollover_mutations
        < tlc
        < replay
        < verus
        < final_release
        < final_marker
    )
    for invocation in (
        "run_sumeragi_v2_applied_phase_admission_mutations.sh",
        "run_sumeragi_v2_indexed_service_activation_mutations.sh",
        "run_sumeragi_v2_adequate_leader_readiness_mutations.sh",
        "run_sumeragi_v2_indexed_height_mutation.sh",
        "run_sumeragi_v2_item_carrier_typing_mutation.sh",
        "run_sumeragi_v2_reply_writer_deadline_mutations.sh",
    ):
        assert source.count(invocation) == 1
    assert source.count("run_sumeragi_v2_typed_rollover_handoff_mutations.sh") == 1
    assert "proof_evidence.json" in source
    cross_requirement = source.index("--print-cross-tool-obligations")
    cross_generation = source.index("--write-cross-tool-evidence")
    assert cross_requirement < tlaps < verus < cross_generation < final_release
    assert 'rm -f -- "$cross_tool_evidence"' in source
    assert '--verus-evidence "$verus_evidence"' in source
    assert '--cross-tool-evidence "$cross_tool_evidence"' in source

    verus_source = (ROOT_DIR / "scripts" / "verify_sumeragi_v2.sh").read_text()
    unit = verus_source.index("--unit")
    fast_network = verus_source.index("--fast-network")
    backend = verus_source.index(
        "run_sumeragi_v2_harness.sh --verus"
    )
    assert unit < fast_network < backend
