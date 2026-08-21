"""Durable Validate lifecycle mutation-corpus source-seal tests."""

from __future__ import annotations

import importlib.util
import shutil
import sys
from pathlib import Path


ROOT_DIR = Path(__file__).resolve().parents[2]
SCRIPT = ROOT_DIR / "scripts" / "formal" / "check_sumeragi_v2_proof_ledger.py"


def load_checker():
    """Load the Sumeragi v2 proof-ledger checker under a stable name."""

    spec = importlib.util.spec_from_file_location(
        "sumeragi_v2_proof_ledger_durable_validate", SCRIPT
    )
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def copy_durable_validate_lifecycle_fixture(
    tmp_path: Path, module
) -> tuple[Path, Path]:
    """Copy the sealed corpus, CI gate, and production refinement seams."""

    repo_root = tmp_path / "repo"
    formal_dir = repo_root / "formal" / "sumeragi_v2"
    formal_dir.mkdir(parents=True)
    for name in module.DURABLE_VALIDATE_LIFECYCLE_MUTATION_FORMAL_ARTIFACTS:
        shutil.copy2(module.FORMAL_DIR / name, formal_dir / name)

    relative_files = (
        module.DURABLE_VALIDATE_LIFECYCLE_MUTATION_RUNNER,
        "ci/check_sumeragi_formal.sh",
        "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
        "crates/iroha_core/src/sumeragi/v2_worker_completion.rs",
        "crates/iroha_core/src/sumeragi/v2_lifecycle_turn_driver.rs",
        "crates/iroha_core/src/sumeragi/v2_lifecycle_validate_sidecar.rs",
        "crates/iroha_core/src/sumeragi/"
        "v2_lifecycle_work_registry_validate_recovery_registry_impl.rs",
        "crates/iroha_core/src/sumeragi/"
        "v2_lifecycle_work_registry_pre_admission.rs",
        "crates/iroha_core/src/sumeragi/"
        "v2_lifecycle_replay_authority_live_wal.rs",
        "crates/iroha_core/src/sumeragi/"
        "v2_lifecycle_body_pipeline_transition.rs",
    )
    for relative in relative_files:
        destination = repo_root / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT_DIR / relative, destination)
    return repo_root, formal_dir


def test_durable_validate_lifecycle_source_seal_covers_corpus_and_production(
    tmp_path: Path,
) -> None:
    """The checked inventory includes one model, six configs, and production."""

    module = load_checker()
    repo_root, formal_dir = copy_durable_validate_lifecycle_fixture(
        tmp_path, module
    )

    assert len(module.DURABLE_VALIDATE_LIFECYCLE_MUTATION_FORMAL_ARTIFACTS) == 7
    assert len(module.DURABLE_VALIDATE_LIFECYCLE_MUTATION_SHA256) == 8
    assert (
        module._durable_validate_lifecycle_mutation_source_fidelity_errors(
            formal_dir, repo_root
        )
        == []
    )


def test_durable_validate_lifecycle_runner_rejects_outcome_mutants(
    tmp_path: Path,
) -> None:
    """Runner roles retain exact statuses, diagnostics, and coverage witnesses."""

    module = load_checker()
    repo_root, _ = copy_durable_validate_lifecycle_fixture(tmp_path, module)
    runner = repo_root / module.DURABLE_VALIDATE_LIFECYCLE_MUTATION_RUNNER
    source = runner.read_text(encoding="utf-8")

    runner.write_text(
        source.replace(
            "durable_validate_lifecycle_fixed.cfg 0 \\",
            "durable_validate_lifecycle_fixed.cfg 12 \\",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._durable_validate_lifecycle_mutation_runner_errors(repo_root)
    assert any("found repaired=0, mutants=6" in error for error in errors), errors

    runner.write_text(
        source.replace(
            "Invariant ExactSidecarWakeReusesWaitingRow is violated.",
            "Invariant GuardedCompletionMatchesClaimedRow is violated.",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._durable_validate_lifecycle_mutation_runner_errors(repo_root)
    assert any(
        "durable Validate lifecycle role "
        "durable_validate_lifecycle_sidecar_new_ordinal_bug.cfg"
        in error
        for error in errors
    ), errors


def test_durable_validate_lifecycle_source_seal_rejects_stale_model(
    tmp_path: Path,
) -> None:
    """The formal model cannot drift without an explicit digest review."""

    module = load_checker()
    repo_root, formal_dir = copy_durable_validate_lifecycle_fixture(
        tmp_path, module
    )
    model = formal_dir / "SumeragiV2DurableValidateLifecycleMutation.tla"
    model.write_text(
        model.read_text(encoding="utf-8") + "\n\\* stale mutation\n",
        encoding="utf-8",
    )

    errors = module._durable_validate_lifecycle_mutation_source_fidelity_errors(
        formal_dir, repo_root
    )
    assert any(
        str(model) in error and "must match exact reviewed SHA-256" in error
        for error in errors
    ), errors


def test_durable_validate_lifecycle_source_seal_rejects_unreserved_dispatch(
    tmp_path: Path,
) -> None:
    """The scheduler cannot publish Validate after discarding its reservation."""

    module = load_checker()
    repo_root, _ = copy_durable_validate_lifecycle_fixture(tmp_path, module)
    scheduler = (
        repo_root
        / "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs"
    )
    source = scheduler.read_text(encoding="utf-8")
    assert source.count("reservation.commit(dispatch);") >= 1
    scheduler.write_text(
        source.replace("reservation.commit(dispatch);", "drop(dispatch);", 1),
        encoding="utf-8",
    )

    errors = (
        module._durable_validate_lifecycle_production_source_fidelity_errors(
            repo_root
        )
    )
    assert any(
        "Ready Validate must reserve the worker slot" in error
        for error in errors
    ), errors


def test_applied_phase_formal_contract_has_no_deleted_validation_callback() -> None:
    """The admission matrix names only the surviving storage callback surface."""

    paths = (
        ROOT_DIR
        / "formal/sumeragi_v2/SumeragiV2AppliedPhaseAdmissionMutation.tla",
        ROOT_DIR
        / "scripts/formal/run_sumeragi_v2_applied_phase_admission_mutations.sh",
        ROOT_DIR / "scripts/formal/sumeragi_v2_admission_mutation_contracts.py",
    )
    for path in paths:
        source = path.read_text(encoding="utf-8")
        assert "ValidationSucceeded" not in source
        assert "validation_succeeded" not in source
        assert "ConflictPolarity" not in source
