# Executed lexically in sumeragi_v2_proof_ledger_test.py; do not collect directly.

KURA_PRODUCTION_COMPONENT_FILES = (
    Path("crates/iroha_core/src/kura/startup_finality_support.rs"),
    Path("crates/iroha_core/src/kura/bound_progress_and_retained_support.rs"),
    Path("crates/iroha_core/src/kura/autonomous_reservation_bounds.rs"),
    Path("crates/iroha_core/src/kura/prune_commit_merge_support.rs"),
    Path("crates/iroha_core/src/kura/replica_advert_and_body_status.rs"),
    Path("crates/iroha_core/src/kura/retained_finality_replica_authority.rs"),
    Path("crates/iroha_core/src/kura/autonomous_merge_bundle_support.rs"),
    Path("crates/iroha_core/src/kura/autonomous_reservation_types.rs"),
    Path("crates/iroha_core/src/kura/autonomous_reservation_inventory.rs"),
    Path("crates/iroha_core/src/kura/autonomous_reservation_classifier.rs"),
    Path("crates/iroha_core/src/kura/historical_autonomous_recovery.rs"),
)
REVIEWED_RUST_INCLUDE_MANIFESTS = {
    Path("crates/iroha_config/src/parameters/actual.rs"): (
        Path("actual/tests.rs"),
    ),
    Path("crates/iroha_config/src/parameters/user.rs"): (
        Path("user/kura.rs"),
        Path("user/kura_and_snapshot_tests.rs"),
    ),
    Path("crates/iroha_core/src/kura.rs"): (
        Path("kura/startup_finality_support.rs"),
        Path("kura/bound_progress_and_retained_support.rs"),
        Path("kura/autonomous_reservation_bounds.rs"),
        Path("kura/prune_commit_merge_support.rs"),
        Path("kura/replica_advert_and_body_status.rs"),
        Path("kura/retained_finality_replica_authority.rs"),
        Path("kura/autonomous_merge_bundle_support.rs"),
        Path("kura/autonomous_reservation_types.rs"),
        Path("kura/autonomous_reservation_inventory.rs"),
        Path("kura/autonomous_reservation_classifier.rs"),
        Path("kura/historical_autonomous_recovery.rs"),
        Path("kura/tests/01_support_snapshot_bootstrap_and_rewrite.rs"),
        Path("kura/tests/01a_retained_eviction_and_rewrite_tail.rs"),
        Path("kura/tests/02_replacement_and_preflight.rs"),
        Path("kura/tests/03_preflight_and_merge_entry.rs"),
        Path("kura/tests/03a_preflight_and_merge_entry_tail.rs"),
        Path("kura/tests/04_merge_log_and_associations.rs"),
        Path("kura/tests/05_merge_resolution_and_eviction.rs"),
        Path("kura/tests/05a_replica_advert_and_body_eviction.rs"),
        Path("kura/tests/06_eviction_and_autonomous_lanes.rs"),
        Path("kura/tests/07a_autonomous_reservation_reconciliation_support.rs"),
        Path("kura/tests/07_autonomous_lanes_and_sidecars.rs"),
        Path("kura/tests/07b_autonomous_reservation_reconciliation_tests.rs"),
        Path("kura/tests/08_lane_receipts_and_artifacts.rs"),
        Path("kura/tests/09_lane_artifacts_and_fastpq.rs"),
        Path("kura/tests/10_native_amx_and_roster.rs"),
        Path("kura/tests/10b_native_amx_prepublication_transition.rs"),
        Path("kura/tests/11_roster_and_progress_sidecars.rs"),
        Path("kura/tests/12_sidecar_index_and_pruning.rs"),
        Path("kura/tests/13_manifests_and_fsync.rs"),
    ),
    Path("crates/iroha_core/src/kura/lane_geometry.rs"): (
        Path("lane_geometry_tests/00_support.rs"),
        Path("lane_geometry/native_amx_retained_window_tests.rs"),
        Path("lane_geometry_tests/00_retirement.rs"),
        Path("lane_geometry_tests/01_retirement_and_recovery.rs"),
        Path("lane_geometry_tests/02_geometry_moves_and_journal.rs"),
        Path("lane_geometry_tests/03_gc_and_startup.rs"),
    ),
    Path("crates/iroha_core/src/snapshot.rs"): (
        Path("snapshot/support_policy_tests.rs"),
        Path("snapshot/write_roundtrip_tests.rs"),
        Path("snapshot/reconciliation_generation_tests.rs"),
    ),
    Path("crates/iroha_core/src/sumeragi/v2_worker.rs"): (
        Path("v2_worker/exact_output_rollover_claim.rs"),
        Path("v2_worker/kura_replica_advert_refresh.rs"),
        Path("tests/v2_worker_reply_route_cases.rs"),
        Path("tests/v2_worker_backpressure_cases.rs"),
        Path("tests/v2_worker_serve_unsealed_cases.rs"),
        Path("tests/v2_worker_serve_decision_restart_cases.rs"),
    ),
    Path("crates/iroha_core/src/sumeragi/v2_runtime.rs"): tuple(
        Path(f"tests/v2_runtime_unsealed_{index:02}.rs") for index in range(7)
    ),
    Path("crates/iroha_core/src/sumeragi/v2_runner.rs"): tuple(
        Path(f"tests/v2_runner_unsealed_{index:02}.rs") for index in range(3)
    ),
    Path("crates/iroha_core/src/sumeragi/v2_apply.rs"): tuple(
        Path(f"tests/v2_apply_unsealed_{index:02}.rs") for index in range(3)
    ),
    Path("crates/iroha_core/src/sumeragi/v2_core/reducer.rs"): (
        Path("tests/v2_core_reducer_primitive_projection.rs"),
    ),
    Path("crates/iroha_core/src/sumeragi/v2_core/tests.rs"): (
        Path("tests/v2_core_view_zero_parent_binding.rs"),
        Path("tests/empty_replay_resume_test.rs"),
    ),
    Path("crates/iroha_core/src/sumeragi/v2_lane_work.rs"): (
        Path("v2_lane_work/canonical_executed_block_application_repair.rs"),
        Path("tests/v2_lane_work_observer_role.rs"),
        Path("tests/v2_lane_work_native_body_recovery.rs"),
        Path("tests/v2_lane_work_effect_queue.rs"),
    ),
    Path("integration_tests/tests/sumeragi_v2_runner.rs"): (
        Path("sumeragi_v2_runner/restart_timing_test.rs"),
    ),
}
def copy_merge_runtime_config_fixture(tmp_path: Path) -> Path:
    """Copy only the config-v6 merge/pending projection and its live consumers."""

    for relative in (
        Path("crates/iroha_config/src/parameters/defaults.rs"),
        Path("crates/iroha_config/src/parameters/actual.rs"),
        Path("crates/iroha_config/src/parameters/user.rs"),
        Path("crates/iroha_core/src/merge_sidecar.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_lane_work.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
    ):
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / relative, destination)
    kura = tmp_path / "crates/iroha_core/src/kura.rs"
    kura_production_includes = "\n".join(
        f'include!("{relative.relative_to("crates/iroha_core/src").as_posix()}");'
        for relative in KURA_PRODUCTION_COMPONENT_FILES
    )
    kura.write_text(
        """
pub fn new_with_configured_lane_catalog_and_snapshot_bootstrap_and_sumeragi_limits() {
    let pending_control_sidecar_limits = PendingControlSidecarLimits::from_config(
        sumeragi_limits,
        &config.store_dir.resolve_relative_path(),
    )?;
}

fn pending_merge_entry_paths_unlocked() {
    if paths.len() == self.pending_control_sidecar_limits.certified_merge_entries {
        return Err(Self::invalid_pending_merge_entry_error(
            directory,
            "pending certified merge entry count exceeds the hard limit",
        ));
    }
}

fn pending_queue_plan_admission_paths_unlocked() {
    if paths.len() == self.pending_control_sidecar_limits.queue_plan_admissions {
        return Err(Self::invalid_pending_queue_plan_admission_error(
            directory,
            "pending QueuePlan admission certificate count exceeds the hard limit",
        ));
    }
}

fn validate_pending_merge_entries_on_startup() {
    if !self
        .pending_control_sidecar_limits
        .combined_bytes_within_limit(merge_bytes, admission_bytes)
    {
        return Err(Self::invalid_pending_queue_plan_admission_error(
            self.store_root.clone(),
            "pending merge and QueuePlan admission sidecars exceed their shared hard byte limit",
        ));
    }
}

pub(crate) fn persist_pending_certified_merge_entry() {
    if paths.len() == self.pending_control_sidecar_limits.certified_merge_entries {
        return Err(Self::invalid_pending_merge_entry_error(
            directory,
            "pending certified merge entry count exceeds the hard limit",
        ));
    }
    if pending_bytes.checked_add(bytes.len()).is_none_or(|total| {
        !self
            .pending_control_sidecar_limits
            .combined_bytes_within_limit(total, admission_bytes)
    }) {
        return Err(error);
    }
}

pub fn persist_pending_queue_plan_admission_certificate() {
    if paths.len() == self.pending_control_sidecar_limits.queue_plan_admissions {
        return Err(Self::invalid_pending_queue_plan_admission_error(
            directory,
            "pending QueuePlan admission certificate count exceeds the hard limit",
        ));
    }
    if admission_bytes
        .checked_add(canonical_certificate_bytes.len())
        .is_none_or(|total| {
            !self
                .pending_control_sidecar_limits
                .combined_bytes_within_limit(merge_bytes, total)
        })
    {
        return Err(error);
    }
}

__KURA_PRODUCTION_INCLUDES__

#[cfg(test)]
mod tests {}
""".replace("__KURA_PRODUCTION_INCLUDES__", kura_production_includes),
        encoding="utf-8",
    )
    for relative in KURA_PRODUCTION_COMPONENT_FILES:
        component = tmp_path / relative
        component.parent.mkdir(parents=True, exist_ok=True)
        component.write_text(
            f"// isolated {component.name} fixture\n",
            encoding="utf-8",
        )
    daemon = tmp_path / "crates/irohad/src/main.rs"
    daemon.parent.mkdir(parents=True, exist_ok=True)
    daemon.write_text(
        """
fn production_startup() {
    Kura::new_with_configured_lane_catalog_and_snapshot_bootstrap_and_sumeragi_limits(
        &config.kura,
        &config.nexus.lane_config,
        &config.nexus.configured_lane_catalog,
        &config.snapshot.bootstrap,
        &config.sumeragi.limits,
    );
}
""",
        encoding="utf-8",
    )
    return tmp_path


def merge_runtime_config_errors(repo_root: Path) -> list[str]:
    """Run one mutation check in a fresh process so large Rust tokens are released."""

    probe = subprocess.run(
        [
            sys.executable,
            "-c",
            """
import importlib.util
import json
import sys
from pathlib import Path

spec = importlib.util.spec_from_file_location("merge_runtime_checker", sys.argv[1])
assert spec is not None and spec.loader is not None
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)
print(json.dumps(module._merge_runtime_config_production_source_fidelity_errors(
    Path(sys.argv[2])
)))
""",
            str(SCRIPT),
            str(repo_root),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    assert probe.returncode == 0, probe.stderr
    errors = json.loads(probe.stdout)
    assert isinstance(errors, list) and all(isinstance(error, str) for error in errors)
    return errors


def test_ledger_is_canonical_json() -> None:
    module = load_checker()
    source = module.LEDGER_PATH.read_text(encoding="utf-8")
    parsed = json.loads(source)

    assert source == json.dumps(parsed, indent=2, ensure_ascii=False) + "\n"


def copy_audited_rank_leaf_contract_fixture(tmp_path: Path, module) -> Path:
    """Install the reviewed Stage-4/5 contracts around the current proof source."""

    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    vocabulary_source = vocabulary.read_text(encoding="utf-8")
    property_block = r'''
ProtectedStage4RankProgressProperty(specification) ==
  specification
    => \A candidate \in AsyncCandidateSet, position \in Nat:
         (gst
           /\ ResponsiveProtectedCandidateOwned(candidate)
           /\ CandidateServiceRank(candidate) = <<4, position>>)
           ~> (~ResponsiveProtectedCandidateOwned(candidate)
                \/ ServiceRankLess(CandidateServiceRank(candidate),
                     <<4, position>>))

ProtectedStage5RankProgressProperty(specification) ==
  specification
    => \A candidate \in AsyncCandidateSet, position \in Nat:
         (gst
           /\ ResponsiveProtectedCandidateOwned(candidate)
           /\ CandidateServiceRank(candidate) = <<5, position>>)
           ~> (~ResponsiveProtectedCandidateOwned(candidate)
                \/ ServiceRankLess(CandidateServiceRank(candidate),
                     <<5, position>>))
'''
    if "ProtectedStage4RankProgressProperty" not in vocabulary_source:
        vocabulary_source = vocabulary_source.replace(
            "=============================================================================\n",
            property_block + "\n=============================================================================\n",
            1,
        )
        vocabulary.write_text(vocabulary_source, encoding="utf-8")

    proof = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    proof_source = proof.read_text(encoding="utf-8")
    wrapper_block = r'''
THEOREM ProtectedStage4RankProgressFromFairScheduler ==
  \A initialContext:
    ProtectedStage4RankProgressProperty(AsyncSpecAt(initialContext))
BY FairProtectedStage4RankDescent
   DEF ProtectedStage4RankProgressProperty

THEOREM ProtectedStage5RankProgressFromFairFifo ==
  \A initialContext:
    ProtectedStage5RankProgressProperty(AsyncSpecAt(initialContext))
BY FairProtectedStage5RankDescent
   DEF ProtectedStage5RankProgressProperty
'''
    if "ProtectedStage4RankProgressFromFairScheduler" not in proof_source:
        proof_source = proof_source.replace(
            "=============================================================================\n",
            wrapper_block + "\n=============================================================================\n",
            1,
        )
        proof.write_text(proof_source, encoding="utf-8")
    return formal_dir


def audited_rank_leaf_contract_errors(module, formal_dir: Path) -> list[str]:
    """Run both source and ledger-target guards for the audited rank leaves."""

    proof_source = (
        formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    ).read_text(encoding="utf-8")
    errors = module._async_proof_architecture_errors(formal_dir)
    errors.extend(
        module._proof_obligation_architecture_errors(
            module.load_ledger()["obligations"],
            {"SumeragiV2AsyncLivenessProofs": proof_source},
        )
    )
    return errors


def test_audited_rank_leaf_synthetic_contract_is_green(tmp_path: Path) -> None:
    module = load_checker()
    formal_dir = copy_audited_rank_leaf_contract_fixture(tmp_path, module)

    assert audited_rank_leaf_contract_errors(module, formal_dir) == []


@pytest.mark.parametrize(
    ("filename", "kind", "symbol", "old", "new", "expected_error"),
    (
        (
            "SumeragiV2AsyncLivenessProofs.tla",
            "theorem",
            "AsyncSpecAlwaysProgressOwnershipInvariant",
            "AsyncSpecAt(initialContext) => []AsyncProgressOwnershipInvariant",
            "AsyncSpecAt(initialContext) => <>AsyncProgressOwnershipInvariant",
            "AsyncSpecAlwaysProgressOwnershipInvariant must state only",
        ),
        (
            "SumeragiV2AsyncLivenessProofs.tla",
            "theorem",
            "AsyncSpecAlwaysProgressOwnershipInvariant",
            "AsyncBracketNextPreservesProgressOwnership",
            "AsyncBracketNextPreservesStrongTypeInvariant",
            "omits explicit transition/fairness inventory",
        ),
        (
            "SumeragiV2LivenessProofs.tla",
            "operator",
            "ProtectedStage4RankProgressProperty",
            "CandidateServiceRank(candidate) = <<4, position>>",
            "CandidateServiceRank(candidate) = <<5, position>>",
            "ProtectedStage4RankProgressProperty must equal only",
        ),
        (
            "SumeragiV2AsyncLivenessProofs.tla",
            "theorem",
            "ProtectedStage4RankProgressFromFairScheduler",
            "ProtectedStage4RankProgressProperty(AsyncSpecAt(initialContext))",
            "ProtectedStage4RankProgressProperty(AsyncFiniteSpec)",
            "ProtectedStage4RankProgressFromFairScheduler must state only",
        ),
        (
            "SumeragiV2AsyncLivenessProofs.tla",
            "theorem",
            "ProtectedStage4RankProgressFromFairScheduler",
            "BY FairProtectedStage4RankDescent",
            "BY PTL",
            "omits explicit transition/fairness inventory",
        ),
        (
            "SumeragiV2LivenessProofs.tla",
            "operator",
            "ProtectedStage5RankProgressProperty",
            "CandidateServiceRank(candidate) = <<5, position>>",
            "CandidateServiceRank(candidate) = <<4, position>>",
            "ProtectedStage5RankProgressProperty must equal only",
        ),
        (
            "SumeragiV2AsyncLivenessProofs.tla",
            "theorem",
            "ProtectedStage5RankProgressFromFairFifo",
            "ProtectedStage5RankProgressProperty(AsyncSpecAt(initialContext))",
            "ProtectedStage5RankProgressProperty(AsyncFiniteSpec)",
            "ProtectedStage5RankProgressFromFairFifo must state only",
        ),
        (
            "SumeragiV2AsyncLivenessProofs.tla",
            "theorem",
            "ProtectedStage5RankProgressFromFairFifo",
            "BY FairProtectedStage5RankDescent",
            "BY PTL",
            "omits explicit transition/fairness inventory",
        ),
    ),
)
def test_audited_rank_leaf_source_mutations_fail_closed(
    tmp_path: Path,
    filename: str,
    kind: str,
    symbol: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_audited_rank_leaf_contract_fixture(tmp_path, module)
    path = formal_dir / filename
    source = path.read_text(encoding="utf-8")
    mutator = mutate_tla_operator if kind == "operator" else mutate_tla_theorem
    path.write_text(mutator(source, symbol, old, new), encoding="utf-8")

    errors = audited_rank_leaf_contract_errors(module, formal_dir)
    assert any(
        expected_error in error and symbol in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("symbol", "token"),
    (
        (
            "ProtectedServeStage5CarrierFacts",
            "ServeOccurrenceIndexCharacterization",
        ),
        (
            "ProtectedServeStage5EnablesFairWorker",
            "QueuedIoEnablesPostGstService",
        ),
        (
            "ProtectedServeStage5WorkerStrictlyProgresses",
            "TailRemovesUniqueServeOccurrence",
        ),
        (
            "ProtectedServeStage5UnlessProgress",
            "AsyncBracketNextPreservesStrongTypeInvariant",
        ),
        (
            "FairProtectedServeStage5RankDescent",
            "ProtectedServeStage5EnablesFairWorker",
        ),
        (
            "ProtectedServeRankProgressFromFairFifo",
            "FairProtectedServeStage5RankDescent",
        ),
    ),
)
def test_protected_serve_fifo_proof_dependency_mutations_fail_closed(
    tmp_path: Path,
    symbol: str,
    token: str,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    proof = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    proof.write_text(
        delete_tla_theorem_token(
            proof.read_text(encoding="utf-8"),
            symbol,
            token,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        symbol in error
        and "omits explicit transition/fairness inventory" in error
        and token in error
        for error in errors
    ), errors


def test_serve_occurrence_rank_and_starvation_conjunct_are_pinned(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    vocabulary.write_text(
        source.replace(
            "ServeJobRank(node, job) == <<5, ServeJobIndex(node, job)>>",
            "ServeJobRank(node, job) == <<5, CandidateIoIndex("
            "job.candidate, asyncIoQueues[node])>>",
            1,
        ).replace(
            "     \\/ ProtectedServeRankDecreaseStep\n",
            "",
            1,
        ).replace(
            "  /\\ ProtectedServeStarvationProperty(specification)\n",
            "",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any("ServeJobRank must equal only" in error for error in errors)
    assert any("PostGstProductiveStep must equal only" in error for error in errors)
    assert any("StarvationFreedomProperty must equal only" in error for error in errors)


def test_exact_removal_and_protected_slot_geometry_theorems_are_pinned(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    proofs = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    source = proofs.read_text(encoding="utf-8")
    removal = source.index("THEOREM OneRemovalIncreasesSourceProtectionByAtMostOne")
    universe = source.index("THEOREM ProtectedProgressSlotUniverseSize")
    mutated = (
        source[:removal]
        + source[removal:universe].replace(
            "LET after == SequenceWithoutIndex(before, selected)",
            "LET after == Tail(before)",
            1,
        )
        + source[universe:].replace(
            "Cardinality(ProtectedProgressSlotUniverse) = 2 * N + 3",
            "Cardinality(ProtectedProgressSlotUniverse) = N + 3",
            1,
        )
    )
    proofs.write_text(mutated, encoding="utf-8")

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "OneRemovalIncreasesSourceProtectionByAtMostOne must state only" in error
        for error in errors
    )
    assert any(
        "ProtectedProgressSlotUniverseSize must state only" in error
        for error in errors
    )


def test_normal_proposal_prepare_protection_contract_is_pinned(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    vocabulary.write_text(
        source.replace(
            "     \\/ NormalProposalPrepareCandidate(candidate)\n", "", 1
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "ProtectedServiceCandidate must equal only" in error
        for error in errors
    )


def test_normal_proposal_prepare_kind_inventory_is_pinned(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    vocabulary.write_text(
        source.replace(
            '{"Proposal", "PrepareVote", "CommitVote"}',
            '{"Proposal", "PrepareVote"}',
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "NormalProposalPrepareNetworkKinds must equal only" in error
        for error in errors
    )


def test_normal_proposal_prepare_requires_canonical_carrier(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    vocabulary.write_text(
        source.replace(
            "ProtectedServiceCandidate(candidate) ==\n"
            "  /\\ candidate \\in AsyncCandidateSet\n",
            "ProtectedServiceCandidate(candidate) ==\n"
            "  /\\ AsyncCandidateTyped(candidate)\n",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "ProtectedServiceCandidate must equal only" in error
        for error in errors
    )


def test_normal_delivery_class_is_frozen_at_admission(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    frozen_network = (
        "    /\\ candidate = FrozenNormalDeliveryCandidate(\n"
        "                     item, consumerContext, consumerView,\n"
        "                     consumerGeneration)\n"
    )
    assert frozen_network in source
    vocabulary.write_text(
        source.replace(
            frozen_network,
            "    /\\ candidate = NormalDeliveryCandidate(item)\n",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "NormalProposalPrepareNetworkCandidate must equal only" in error
        for error in errors
    )

    frozen_identity = (
        "       consumerContext, consumerView, consumerGeneration, item,\n"
    )
    assert frozen_identity in source
    vocabulary.write_text(
        source.replace(
            frozen_identity,
            "       context, nodeView[item.envelope.recipient],\n"
            "       generation[item.envelope.recipient], item,\n",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "FrozenNormalDeliveryCandidate must equal only" in error
        for error in errors
    )


def test_normal_install_successor_is_required_and_frozen(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    install_successor_branch = (
        "     \\/ \\E command \\in AsyncCandidateSet,\n"
        "            installedContext \\in ContextRecords,\n"
        "            priorGeneration \\in Generations,\n"
        "            subject \\in SubjectOrNone:\n"
        "          /\\ command.kind = \"PersistInstallTC\"\n"
        "          /\\ command.view + 1 \\in Views\n"
        "          /\\ candidate = FrozenInstallProposalSuccessor(\n"
        "                           command, installedContext,\n"
        "                           priorGeneration, subject)\n"
    )
    assert install_successor_branch in source
    vocabulary.write_text(
        source.replace(install_successor_branch, "", 1),
        encoding="utf-8",
    )
    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "NormalProposalPrepareNoItemCandidate must equal only" in error
        for error in errors
    )

    frozen_generation = "NextCandidateGeneration(priorGeneration)"
    assert frozen_generation in source
    vocabulary.write_text(
        source.replace(
            frozen_generation,
            "generation[command.node]",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "FrozenInstallProposalSuccessor must equal only" in error
        for error in errors
    )


def test_begin_prepare_parent_inventory_is_pinned(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    vocabulary.write_text(
        source.replace(
            '{"DeliverProposal", "ValidateBody"}',
            '{"DeliverProposal"}',
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "NormalBeginPrepareParentKinds must equal only" in error
        for error in errors
    )


def test_normal_candidate_step_stability_theorem_is_pinned(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    proofs = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    source = proofs.read_text(encoding="utf-8")
    proofs.write_text(
        source.replace(
            "    /\\ AsyncNext\n"
            "    => NormalProposalPrepareCandidate(candidate)'\n",
            "    /\\ PostGstSchedulerActionEnabled\n"
            "    => NormalProposalPrepareCandidate(candidate)'\n",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "AsyncNextPreservesNormalProposalPrepareCandidate must state only"
        in error
        for error in errors
    )


def test_deadlock_contract_rejects_scheduler_only_enablement(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    property_offset = source.index("DeadlockFreedomProperty(specification) ==")
    enabled_offset = source.index(
        "PostGstProductiveActionEnabled", property_offset
    )
    vocabulary.write_text(
        source[:enabled_offset]
        + source[enabled_offset:].replace(
            "PostGstProductiveActionEnabled",
            "PostGstSchedulerActionEnabled",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "DeadlockFreedomProperty must equal only" in error
        for error in errors
    )


def test_deadlock_contract_rejects_scheduler_only_productive_alias(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    vocabulary.write_text(
        source.replace(
            "PostGstProductiveActionEnabled == ENABLED PostGstProductiveStep",
            "PostGstProductiveActionEnabled == PostGstSchedulerActionEnabled",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "PostGstProductiveActionEnabled must equal only" in error
        for error in errors
    )


def test_async_source_fidelity_pins_dual_progress_ingress_geometry(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")
    source = (module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla").read_text(
        encoding="utf-8"
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    path.write_text(
        source.replace(
            '   "TimeoutCertificate", "CertifiedRequest", "CommitCertificateRequest",\n',
            '   "TimeoutCertificate", "CommitCertificateRequest",\n',
            1,
        ).replace(
            "    + Cardinality(\n"
            "        IngressTransportCompletionProtectedSourcesFor(lanes, recipient))\n",
            "",
            1,
        ).replace(
            'IngressTransportCompletionKinds == {"Chunk", "CertifiedResponse"}',
            'IngressTransportCompletionKinds == {"Chunk"}',
            1,
        ).replace(
            "  \\/ ~IngressLaneHasTransportCompletionIn(\n"
            "       asyncIngressLanes, item.envelope.recipient, item.source)\n",
            "  \\/ TRUE\n",
            1,
        ).replace(
            '                    "TimeoutCertificate", "Chunk", "CertifiedResponse",\n'
            '                    "CommitCertificateResponse",\n',
            '                    "TimeoutCertificate", "Chunk", "CertifiedRequest",\n'
            '                    "CertifiedResponse", "CommitCertificateRequest",\n'
            '                    "CommitCertificateResponse",\n',
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "IngressTransportCompletionKinds must equal only" in error
        for error in errors
    )
    assert any("IngressProgressKinds must equal only" in error for error in errors)
    assert any(
        "IngressProtectedSlotCountFor must equal only" in error for error in errors
    )
    assert any(
        "AsyncTransportCompletionOwnerGateAllows must equal only" in error
        for error in errors
    )
    assert any("DeliveryClass must equal only" in error for error in errors)


def test_async_source_fidelity_pins_untrusted_transport_completion_exclusion(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")
    source = (module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla").read_text(
        encoding="utf-8"
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    path.write_text(
        source.replace(
            '          kind \\in {"Chunk", "CertifiedResponse"},\n'
            "          source \\in AsyncIngressSources,\n",
            '          kind \\in {"Chunk", "CertifiedResponse"},\n'
            "          source \\in ValidatorIds,\n",
            1,
        )
        .replace(
            '  /\\ (item.kind \\notin {"Noise", "Chunk", "CertifiedResponse"}\n'
            "        => item.source \\in ValidatorIds)",
            '  /\\ (item.kind # "Noise" => item.source \\in ValidatorIds)',
            1,
        )
        .replace(
            "  IN /\\ kind \\in IngressTransportCompletionKinds\n",
            '  IN /\\ kind = "Chunk"\n',
            1,
        )
        .replace("     /\\ nonce = 0\n", "", 1)
        .replace(
            "       InjectUntrustedTransportCompletion(kind, recipient, nonce)\n",
            "       TRUE\n",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "AsyncNetworkItems omits required production behavior" in error
        for error in errors
    )
    assert any(
        "AsyncItemTyped omits required production behavior" in error
        for error in errors
    )
    assert any(
        "InjectUntrustedTransportCompletion omits required production behavior"
        in error
        for error in errors
    )
    assert any(
        "AsyncFaultStep omits required production behavior" in error
        for error in errors
    )

    path.write_text(
        source.replace("     /\\ nonce = 0\n", "", 1),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "InjectUntrustedTransportCompletion omits required production behavior"
        in error
        for error in errors
    )


def test_async_source_fidelity_pins_timeout_signer_partition_without_displacement(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")
    source = (module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla").read_text(
        encoding="utf-8"
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    path.write_text(
        source.replace(
            "AsyncDeferredProgressCapacity >= 2 * N + 3",
            "AsyncDeferredProgressCapacity >= N + 3",
            1,
        ).replace(
            '    [] command.kind = "DeliverTimeout" ->\n'
            '         command.item.kind = "TimeoutVote"\n',
            "",
            1,
        ).replace(
            "     ELSE IF SameProtectedProgressSlotIndices(node, command) # {}\n"
            "          THEN queue\n",
            "     ELSE IF SameProtectedProgressSlotIndices(node, command) # {}\n"
            "          THEN SequenceWithoutIndex(queue, 1)\n",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "AsyncConfiguration omits required production behavior" in error
        for error in errors
    )
    assert any("ProtectedProgressCommand must equal only" in error for error in errors)
    assert any("DeferredProgressAfter must equal only" in error for error in errors)


@pytest.mark.parametrize(
    ("old", "new"),
    (
        (
            "       /\\ candidate.kind \\in AsyncTimeoutLifecycleKinds\n",
            "       /\\ candidate.causalOrigin.phase "
            "\\in AsyncTimeoutLifecycleKinds\n",
        ),
        (
            "       QueuedCandidates \\cup DeferredCandidates\n"
            "         \\cup CausalCandidates \\cup TrackedWorkCandidates:\n",
            "       QueuedCandidates \\cup DeferredCandidates\n"
            "         \\cup CausalCandidates:\n",
        ),
    ),
)
def test_async_source_fidelity_pins_current_timeout_lifecycle_stage_classifier(
    tmp_path: Path, old: str, new: str
) -> None:
    """Retained timeout origins must not turn proposal successors into clocks."""

    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")
    source = (module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla").read_text(
        encoding="utf-8"
    )
    assert old in source
    (formal_dir / "SumeragiV2AsyncNetwork.tla").write_text(
        source.replace(old, new, 1),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "AsyncOlderOrEqualTimeoutLifecycleOwned must equal only" in error
        for error in errors
    )


def test_async_source_fidelity_pins_live_serve_occurrence_identity(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")
    source = (module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla").read_text(
        encoding="utf-8"
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    path.write_text(
        source.replace(
            'AsyncIoJob("Serve", candidate, FreshAsyncIoServeNonce(node))',
            'AsyncIoJob("Serve", candidate, 0)',
            1,
        ).replace(
            "    /\\ AsyncIoServeNonceOwnership(asyncIoQueues[node])\n",
            "",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)
    assert any("AsyncIoCertifiedServeJob must equal only" in error for error in errors)
    assert any(
        "AsyncIoQueueContentTypeInvariant must equal only" in error
        for error in errors
    )


def copy_timeout_vote_window_fixture(tmp_path: Path, module) -> Path:
    """Copy the bounded TimeoutVote production and regression sources."""

    relatives = (
        Path("crates/iroha_core/src/sumeragi/v2.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core/types.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core/reducer.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core/tests.rs"),
    )
    for relative in relatives:
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(module.ROOT_DIR / relative, destination)
    return tmp_path / relatives[0]


def test_async_source_fidelity_pins_timeout_vote_semantic_capacity_bypass(
    tmp_path: Path,
) -> None:
    """The bounded TimeoutVote production and regression sources are sealed."""

    module = load_checker()
    copy_timeout_vote_window_fixture(tmp_path, module)

    errors = module._timeout_vote_semantic_capacity_source_fidelity_errors(
        tmp_path
    )

    assert errors == []


@pytest.mark.parametrize(
    ("item_name", "old", "new", "expected_error"),
    (
        (
            "admit_authenticated_payload",
            (
                "if !reducer::timeout_vote_view_is_admissible("
                "current_view, vote.round.view)"
            ),
            "if false",
            "current/adjacent view window",
        ),
        (
            "admit_authenticated_payload",
            (
                "locked_commit_progress || matches!(key, "
                "IngressSemanticKey::TimeoutVote { .. })"
            ),
            "locked_commit_progress",
            "bypass only ordinary semantic capacity",
        ),
        (
            "prune_ingress_records",
            (
                "round.height == current_height\n"
                "                        "
                "&& reducer::timeout_vote_view_is_admissible("
                "current_view, round.view)"
            ),
            "round.height == current_height",
            "retained only at the current height and current/adjacent view",
        ),
        (
            "prune_ingress_records",
            (
                "matches_current_lock(*key, record.fingerprint) "
                "|| matches_retained_timeout(*key)"
            ),
            "matches_current_lock(*key, record.fingerprint)",
            "preserve either the exact lock or retained TimeoutVote",
        ),
    ),
)
def test_timeout_vote_semantic_capacity_rejects_real_source_mutations(
    tmp_path: Path,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    """Bounded admission and both protected prune arms fail closed."""

    module = load_checker()
    rust_path = copy_timeout_vote_window_fixture(tmp_path, module)
    mutate_rust_item_source_in_context(
        module,
        rust_path,
        item_name,
        (("impl", "SumeragiV2Adapter"),),
        old,
        new,
    )

    errors = module._timeout_vote_semantic_capacity_source_fidelity_errors(
        tmp_path
    )

    assert any(expected_error in error for error in errors), errors


def test_timeout_vote_semantic_capacity_rejects_two_roster_sets(
    tmp_path: Path,
) -> None:
    """The semantic table reserves lock plus both bounded timeout rounds."""

    module = load_checker()
    rust_path = copy_timeout_vote_window_fixture(tmp_path, module)
    mutate_rust_item_source(
        module,
        rust_path,
        "semantic_ingress_capacity",
        "roster_len.saturating_mul(3)",
        "roster_len.saturating_mul(2)",
    )

    errors = module._timeout_vote_semantic_capacity_source_fidelity_errors(
        tmp_path
    )

    assert any(
        "three roster-bounded protected sets" in error for error in errors
    ), errors


@pytest.mark.parametrize(
    ("old", "new", "expected_error"),
    (
        (
            "FUTURE_TIMEOUT_VOTE_LOOKAHEAD: u64 = 1",
            "FUTURE_TIMEOUT_VOTE_LOOKAHEAD: u64 = 2",
            "lookahead must remain exactly one view",
        ),
        (
            "current_view.saturating_add(FUTURE_TIMEOUT_VOTE_LOOKAHEAD)",
            "current_view.wrapping_add(FUTURE_TIMEOUT_VOTE_LOOKAHEAD)",
            "lower bound and saturating one-view upper bound",
        ),
    ),
)
def test_timeout_vote_view_window_rejects_predicate_mutations(
    tmp_path: Path,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    """The one-round helper cannot widen, wrap, or lose its exact bound."""

    module = load_checker()
    copy_timeout_vote_window_fixture(tmp_path, module)
    types_path = tmp_path / "crates/iroha_core/src/sumeragi/v2_core/types.rs"
    mutate_source_once(types_path, old, new)

    errors = module._timeout_vote_semantic_capacity_source_fidelity_errors(
        tmp_path
    )

    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("item_name", "old", "new", "expected_error"),
    (
        (
            "on_timeout_vote",
            (
                "if !timeout_vote_view_is_admissible("
                "self.durable.current_view(), vote.round().view())"
            ),
            "if false",
            "admission must use the bounded current/adjacent predicate",
        ),
        (
            "on_persisted",
            "self.timeout_votes.retain(|round, _| {",
            "self.timeout_votes.clear();\n                if false {",
            "retain exactly the current/adjacent vote and formed-certificate pools",
        ),
        (
            "on_persisted",
            "self.formed_timeouts.retain(|round| {",
            "self.formed_timeouts.clear();\n                if false {",
            "retain exactly the current/adjacent vote and formed-certificate pools",
        ),
    ),
)
def test_timeout_vote_view_window_rejects_reducer_mutations(
    tmp_path: Path,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    """Reducer admission and both install-retention pools stay bounded."""

    module = load_checker()
    copy_timeout_vote_window_fixture(tmp_path, module)
    reducer_path = tmp_path / "crates/iroha_core/src/sumeragi/v2_core/reducer.rs"
    mutate_rust_item_source_in_context(
        module,
        reducer_path,
        item_name,
        (("impl", "Reducer"),),
        old,
        new,
    )

    errors = module._timeout_vote_semantic_capacity_source_fidelity_errors(
        tmp_path
    )

    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    "test_name",
    (
        "adjacent_future_timeout_votes_form_a_catch_up_certificate",
        "timeout_install_preserves_adjacent_shares_for_the_new_current_view",
        "timeout_votes_beyond_adjacent_lookahead_are_ignored",
    ),
)
def test_timeout_vote_view_window_regressions_cannot_be_deleted(
    tmp_path: Path,
    test_name: str,
) -> None:
    """Catch-up, install preservation, and far-future rejection stay sealed."""

    module = load_checker()
    copy_timeout_vote_window_fixture(tmp_path, module)
    tests_path = tmp_path / "crates/iroha_core/src/sumeragi/v2_core/tests.rs"
    mutate_rust_item_source(
        module,
        tests_path,
        test_name,
        f"fn {test_name}(",
        f"fn removed_{test_name}(",
    )

    errors = module._timeout_vote_semantic_capacity_source_fidelity_errors(
        tmp_path
    )

    assert any(
        f"named {test_name}; found 0" in error for error in errors
    ), errors


@pytest.mark.parametrize(
    "test_name",
    (
        "capacity_bypass_records_follow_current_lock_and_timeout_view",
        "adjacent_future_timeout_vote_remains_retryable_until_current_view_advances",
        "full_normal_deferred_lane_cannot_drop_absolute_timeout",
        "busy_deferred_source_identity_coalesces_across_consumer_view_change",
    ),
)
def test_timeout_vote_semantic_capacity_regressions_cannot_be_deleted(
    tmp_path: Path,
    test_name: str,
) -> None:
    """Capacity, adjacent, full-lane, and cross-view regressions stay exact."""

    module = load_checker()
    rust_path = copy_timeout_vote_window_fixture(tmp_path, module)
    mutate_rust_item_source(
        module,
        rust_path,
        test_name,
        f"fn {test_name}(",
        f"fn removed_{test_name}(",
    )

    errors = module._timeout_vote_semantic_capacity_source_fidelity_errors(
        tmp_path
    )

    assert any(
        f"named {test_name}; found 0" in error for error in errors
    ), errors


MERGE_RUNTIME_PROJECTED_FIELDS = (
    "merge_sidecar_inbound_session_capacity",
    "merge_sidecar_inbound_sessions_per_peer",
    "merge_sidecar_inbound_assembly_bytes",
    "merge_sidecar_inbound_assembly_bytes_per_peer",
    "merge_sidecar_deferred_block_capacity",
    "merge_sidecar_future_block_distance",
    "merge_sidecar_request_timeout_ms",
    "merge_sidecar_outbound_sessions_per_source",
    "merge_sidecar_outbound_bytes_per_source",
    "merge_sidecar_server_request_gates_per_source",
    "pending_certified_merge_entry_capacity",
    "pending_queue_plan_admission_capacity",
    "pending_control_sidecar_bytes",
    "merge_signing_guard_record_capacity",
    "merge_signing_guard_record_bytes",
    "merge_signing_guard_total_bytes",
)


def test_merge_runtime_config_v6_inventory_is_static_and_current() -> None:
    module = load_checker()
    checker_source = SCRIPT.read_text(encoding="utf-8")

    assert tuple(
        Path("crates/iroha_core/src") / relative
        for relative in module._KURA_PRODUCTION_COMPONENT_FILES
    ) == KURA_PRODUCTION_COMPONENT_FILES
    assert tuple(
        projected_field
        for projected_field, *_rest in module.MERGE_RUNTIME_CONFIG_FIELDS
    ) == MERGE_RUNTIME_PROJECTED_FIELDS
    assert len(module.MERGE_RUNTIME_CONFIG_FIELDS) == 16
    assert (
        checker_source.count(
            '"pub const SUMERAGI_V2_CONFIG_FORMAT_VERSION: u16 = 6;"'
        )
        == 2
    )
    assert (
        '"pub const SUMERAGI_V2_CONFIG_FORMAT_VERSION: u16 = 3;"'
        not in checker_source
    )


def test_merge_runtime_config_v6_source_binding_accepts_repository() -> None:
    module = load_checker()

    assert module._merge_runtime_config_production_source_fidelity_errors() == []


def test_reviewed_rust_include_manifests_are_static_and_current() -> None:
    module = load_checker()
    observed = {
        Path(parent): tuple(Path(component) for component in components)
        for parent, components in module._REVIEWED_RUST_INCLUDE_MANIFESTS.items()
    }
    assert observed == REVIEWED_RUST_INCLUDE_MANIFESTS
    assert module._reviewed_rust_include_manifest_errors() == []


@pytest.mark.parametrize("parent_relative", REVIEWED_RUST_INCLUDE_MANIFESTS)
def test_each_reviewed_rust_include_manifest_fails_closed(
    tmp_path: Path,
    parent_relative: Path,
) -> None:
    module = load_checker()
    repo_root = tmp_path / "repo"
    parent = repo_root / parent_relative
    parent.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(ROOT_DIR / parent_relative, parent)
    components = REVIEWED_RUST_INCLUDE_MANIFESTS[parent_relative]
    for component_relative in components:
        destination = parent.parent / component_relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(
            (ROOT_DIR / parent_relative).parent / component_relative,
            destination,
        )

    relative = parent_relative.as_posix()
    errors: list[str] = []
    _path, expanded = module._read_reviewed_rust_source(
        repo_root,
        relative,
        errors,
        "negative-control split source",
    )
    assert errors == []
    assert expanded

    first_component = parent.parent / components[0]
    canonical_component = first_component.read_text(encoding="utf-8")
    first_component.unlink()
    errors = []
    module._read_reviewed_rust_source(
        repo_root,
        relative,
        errors,
        "negative-control split source",
    )
    assert any(
        str(first_component) in error and "regular non-symlink file" in error
        for error in errors
    ), errors

    substitute = first_component.with_name(
        f"{first_component.stem}_symlink_substitute.rs"
    )
    substitute.write_text(canonical_component, encoding="utf-8")
    first_component.symlink_to(substitute.name)
    errors = []
    module._read_reviewed_rust_source(
        repo_root,
        relative,
        errors,
        "negative-control split source",
    )
    assert any(
        str(first_component) in error and "regular non-symlink file" in error
        for error in errors
    ), errors

    first_component.unlink()
    first_component.write_text(canonical_component, encoding="utf-8")
    canonical_parent = parent.read_text(encoding="utf-8")
    canonical_include = f'include!("{components[0].as_posix()}");'
    substituted_include = 'include!("substituted_manifest_component.rs");'
    assert canonical_parent.count(canonical_include) == 1
    parent.write_text(
        canonical_parent.replace(canonical_include, substituted_include, 1),
        encoding="utf-8",
    )
    (parent.parent / "substituted_manifest_component.rs").write_text(
        canonical_component,
        encoding="utf-8",
    )
    errors = []
    module._read_reviewed_rust_source(
        repo_root,
        relative,
        errors,
        "negative-control split source",
    )
    assert any("reviewed Rust include inventory must equal" in error for error in errors)

    parent.write_text(
        canonical_parent + '\ninclude!("extra_manifest_component.rs");\n',
        encoding="utf-8",
    )
    (parent.parent / "extra_manifest_component.rs").write_text(
        "// extra split source\n",
        encoding="utf-8",
    )
    errors = []
    module._read_reviewed_rust_source(
        repo_root,
        relative,
        errors,
        "negative-control split source",
    )
    assert any("reviewed Rust include inventory must equal" in error for error in errors)


@pytest.mark.parametrize("component_relative", KURA_PRODUCTION_COMPONENT_FILES)
def test_kura_production_inventory_rejects_missing_and_symlinked_components(
    tmp_path: Path,
    component_relative: Path,
) -> None:
    module = load_checker()
    repo_root = copy_merge_runtime_config_fixture(tmp_path)
    _path, _source, _components, errors = module._kura_production_source_inventory(
        repo_root
    )
    assert errors == []

    component = repo_root / component_relative
    canonical = component.read_text(encoding="utf-8")
    component.unlink()
    _path, _source, _components, errors = module._kura_production_source_inventory(
        repo_root
    )
    assert any(
        str(component) in error and "regular non-symlink file" in error
        for error in errors
    ), errors

    substitute = component.with_name(f"{component.stem}_substitute.rs")
    substitute.write_text(canonical, encoding="utf-8")
    component.symlink_to(substitute.name)
    _path, _source, _components, errors = module._kura_production_source_inventory(
        repo_root
    )
    assert any(
        str(component) in error and "regular non-symlink file" in error
        for error in errors
    ), errors


def test_kura_production_inventory_rejects_substituted_and_extra_includes(
    tmp_path: Path,
) -> None:
    module = load_checker()
    repo_root = copy_merge_runtime_config_fixture(tmp_path)
    kura_path = repo_root / "crates/iroha_core/src/kura.rs"
    canonical = kura_path.read_text(encoding="utf-8")
    expected = 'include!("kura/startup_finality_support.rs");'
    substituted = 'include!("kura/substituted_finality_support.rs");'
    assert canonical.count(expected) == 1
    kura_path.write_text(
        canonical.replace(expected, substituted, 1),
        encoding="utf-8",
    )
    extra = kura_path.parent / "kura/substituted_finality_support.rs"
    extra.write_text("// substituted production component\n", encoding="utf-8")
    _path, _source, _components, errors = module._kura_production_source_inventory(
        repo_root
    )
    assert any(
        "direct production include inventory must equal" in error for error in errors
    ), errors

    marker = "#[cfg(test)]\nmod tests {}"
    assert canonical.count(marker) == 1
    extra_include = 'include!("kura/extra_production_support.rs");\n\n'
    kura_path.write_text(
        canonical.replace(marker, extra_include + marker, 1),
        encoding="utf-8",
    )
    (kura_path.parent / "kura/extra_production_support.rs").write_text(
        "// extra production component\n",
        encoding="utf-8",
    )
    _path, _source, _components, errors = module._kura_production_source_inventory(
        repo_root
    )
    assert any(
        "direct production include inventory must equal" in error for error in errors
    ), errors


@pytest.mark.parametrize("component_relative", KURA_PRODUCTION_COMPONENT_FILES)
def test_merge_runtime_rejects_retired_ttl_hidden_in_each_kura_component(
    tmp_path: Path,
    component_relative: Path,
) -> None:
    repo_root = copy_merge_runtime_config_fixture(tmp_path)
    component = repo_root / component_relative
    component.write_text(
        component.read_text(encoding="utf-8")
        + "\nconst MERGE_SIDECAR_SERVER_REQUEST_GATE_TTL_MS: u64 = 1;\n",
        encoding="utf-8",
    )
    errors = merge_runtime_config_errors(repo_root)
    assert any(
        str(component) in error
        and "retired wall-clock sidecar gate TTL must remain absent from production"
        in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("relative", "injected"),
    (
        (
            Path("crates/iroha_config/src/parameters/actual.rs"),
            "\nfn retired_ttl_config_mutant() {\n"
            "    let merge_sidecar_server_request_gate_ttl_ms = 1_u64;\n"
            "    drop(merge_sidecar_server_request_gate_ttl_ms);\n"
            "}\n",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
            "\nfn retired_ttl_runner_mutant() {\n"
            "    let merge_sidecar_server_request_gate_ttl = "
            "core::time::Duration::from_secs(1);\n"
            "    drop(merge_sidecar_server_request_gate_ttl);\n"
            "}\n",
        ),
        (
            Path("crates/iroha_core/src/merge_sidecar.rs"),
            "\nconst SERVER_REQUEST_GATE_TTL: u64 = 1;\n",
        ),
    ),
)
def test_merge_runtime_config_v6_rejects_reintroduced_wall_clock_gate_ttl(
    tmp_path: Path,
    relative: Path,
    injected: str,
) -> None:
    repo_root = copy_merge_runtime_config_fixture(tmp_path)
    path = repo_root / relative
    canonical_source = path.read_text(encoding="utf-8")
    module = load_checker()
    assert (
        module._retired_sidecar_gate_ttl_source_errors(
            path,
            canonical_source,
            str(relative),
        )
        == []
    )
    path.write_text(
        canonical_source + injected,
        encoding="utf-8",
    )

    errors = module._retired_sidecar_gate_ttl_source_errors(
        path,
        path.read_text(encoding="utf-8"),
        str(relative),
    )

    assert any(
        "retired wall-clock sidecar gate TTL must remain absent from production"
        in error
        for error in errors
    ), errors


@pytest.mark.parametrize("field", MERGE_RUNTIME_PROJECTED_FIELDS)
def test_merge_runtime_config_v6_rejects_each_projection_field_substitution(
    tmp_path: Path,
    field: str,
) -> None:
    repo_root = copy_merge_runtime_config_fixture(tmp_path)
    actual_path = repo_root / "crates/iroha_config/src/parameters/actual.rs"
    source = actual_path.read_text(encoding="utf-8")
    projection_start = source.index("limits: SumeragiV2Limits {")
    projection_end = source.index(
        "native_amx_signing_guard_record_capacity,", projection_start
    )
    needle = f"                {field},"
    position = source.index(needle, projection_start, projection_end)
    replacement = f"                {field}: 0,"
    actual_path.write_text(
        source[:position] + replacement + source[position + len(needle) :],
        encoding="utf-8",
    )

    errors = merge_runtime_config_errors(repo_root)

    assert any(
        "shared fingerprint projection carries all 16 config-v6 merge fields"
        in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("relative", "region", "old", "new", "expected_error"),
    (
        (
            "crates/iroha_config/src/parameters/actual.rs",
            "pub const SUMERAGI_V2_CONFIG_FORMAT_VERSION:",
            "pub const SUMERAGI_V2_CONFIG_FORMAT_VERSION: u16 = 6;",
            "pub const SUMERAGI_V2_CONFIG_FORMAT_VERSION: u16 = 5;",
            "merge-runtime shared-config format version 6",
        ),
        (
            "crates/iroha_config/src/parameters/defaults.rs",
            "pub const V2_MERGE_SIDECAR_INBOUND_SESSION_CAPACITY:",
            "V2_MERGE_SIDECAR_INBOUND_SESSION_CAPACITY",
            "V2_RETIRED_MERGE_SIDECAR_INBOUND_SESSION_CAPACITY",
            "config-v6 default V2_MERGE_SIDECAR_INBOUND_SESSION_CAPACITY",
        ),
        (
            "crates/iroha_config/src/parameters/defaults.rs",
            "pub const V2_MERGE_SIGNING_GUARD_METADATA_HEADROOM_BYTES:",
            "V2_MERGE_SIGNING_GUARD_METADATA_HEADROOM_BYTES",
            "V2_RETIRED_MERGE_SIGNING_GUARD_METADATA_HEADROOM_BYTES",
            "merge-signing metadata headroom has one named config source",
        ),
        (
            "crates/iroha_config/src/parameters/user.rs",
            "pub struct SumeragiV2RuntimeLimits {",
            "defaults::sumeragi::V2_MERGE_SIDECAR_INBOUND_SESSION_CAPACITY",
            "defaults::sumeragi::V2_MERGE_SIDECAR_INBOUND_SESSIONS_PER_PEER",
            "user config field merge_sidecar_inbound_session_capacity",
        ),
        (
            "crates/iroha_config/src/parameters/user.rs",
            "limits: actual::SumeragiV2RuntimeLimits {",
            ".merge_sidecar_inbound_session_capacity,",
            ".merge_sidecar_inbound_sessions_per_peer,",
            "user parsing maps all 16 config-v6 merge fields without substitution",
        ),
        (
            "crates/iroha_config/src/parameters/actual.rs",
            "let merge_sidecar_inbound_session_capacity = canonical_bounded_size(",
            "merge_sidecar_inbound_sessions_per_peer,\n"
            "            merge_sidecar_inbound_session_capacity,",
            "merge_sidecar_inbound_sessions_per_peer,\n"
            "            merge_sidecar_inbound_sessions_per_peer,",
            "config validation preserves decided and ordinary inbound session corridors",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "let merge_sidecar_limits = MergeSidecarLimits::new(",
            "non_zero(config.limits.merge_sidecar_inbound_sessions_per_peer)?",
            "non_zero(config.limits.merge_sidecar_inbound_session_capacity)?",
            "runner constructs live sidecar and signing limits from all projected merge fields",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "None => MergeSidecarTransport::open_durable_with_server_stream_capacity(",
            "limits.merge_sidecar_limits,",
            "MergeSidecarLimits::defaults(),",
            "adapter must derive the canonical responder roster and restore or open only its exact durable source, stream, and roster geometry",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "let mut adapter = Self {",
            "merge_sidecars,\n"
            "            exact_output_handoff_owner,\n"
            "            authenticated_merge_qcs:",
            "merge_sidecars,\n"
            "            authenticated_merge_qcs:",
            "adapter hands the exact rehydrated sidecar transport into the live production field",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn with_limits_and_server_stream_capacity(",
            "Self::derive_server_request_capacities(\n"
            "                reply_source_capacity,\n"
            "                limits,\n"
            "                server_stream_capacity,\n"
            "            )?",
            "Self::derive_server_request_capacities(\n"
            "                reply_source_capacity,\n"
            "                limits,\n"
            "                MAX_CERTIFIED_MERGE_SEMANTIC_PEERS,\n"
            "            )?",
            "live sidecar transport derives checked source-partition capacities",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "let merge_signing_guard = MergeSigningGuard::open_with_committed_frontier(",
            "limits.merge_signing_guard_limits,",
            "MergeSigningGuardLimits::defaults(),",
            "adapter opens the durable merge-signing journal with fingerprinted limits",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn defer_block_with_priority(",
            "self.limits.future_block_distance",
            "u64::MAX",
            "live sidecar carrier admission consumes configured future distance",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "if bytes.len() > self.limits.max_record_bytes",
            "total > self.limits.max_total_bytes",
            "total > usize::MAX",
            "merge-signing authorization consumes configured aggregate bytes",
        ),
        (
            "crates/irohad/src/main.rs",
            "Kura::new_with_configured_lane_catalog_and_snapshot_bootstrap_and_sumeragi_limits(",
            "&config.sumeragi.limits,",
            "&iroha_config::parameters::actual::SumeragiV2RuntimeLimits::default(),",
            "daemon passes fingerprinted pending-control limits into production Kura",
        ),
        (
            "crates/iroha_core/src/kura.rs",
            "let pending_control_sidecar_limits = PendingControlSidecarLimits::from_config(",
            "sumeragi_limits,",
            "&SumeragiV2RuntimeLimits::default(),",
            "Kura validates pending-control limits before opening its store",
        ),
        (
            "crates/iroha_core/src/kura.rs",
            "pub(crate) fn persist_pending_certified_merge_entry(",
            "paths.len() == self.pending_control_sidecar_limits.certified_merge_entries",
            "paths.len() == usize::MAX",
            "Kura merge admission consumes the configured pending-entry count",
        ),
        (
            "crates/iroha_core/src/kura.rs",
            "pub fn persist_pending_queue_plan_admission_certificate(",
            "paths.len() == self.pending_control_sidecar_limits.queue_plan_admissions",
            "paths.len() == usize::MAX",
            "Kura QueuePlan admission consumes the configured certificate count",
        ),
        (
            "crates/iroha_core/src/kura.rs",
            "fn validate_pending_merge_entries_on_startup(",
            ".combined_bytes_within_limit(merge_bytes, admission_bytes)",
            ".merge_bytes_within_limit(merge_bytes)",
            "Kura startup consumes the configured shared pending byte limit",
        ),
    ),
)
def test_merge_runtime_config_v6_rejects_disconnected_production_seams(
    tmp_path: Path,
    relative: str,
    region: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    repo_root = copy_merge_runtime_config_fixture(tmp_path)
    path = repo_root / relative
    source = path.read_text(encoding="utf-8")
    region_start = source.index(region)
    mutation = source.index(old, region_start)
    path.write_text(
        source[:mutation] + new + source[mutation + len(old) :],
        encoding="utf-8",
    )

    errors = merge_runtime_config_errors(repo_root)

    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("relative", "item_name", "old", "new", "expected_error"),
    (
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "check_queue_limit",
            ".checked_add(frame_len)",
            ".saturating_add(frame_len)",
            "checked byte/frame queue admission and overflow rejection",
        ),
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "encrypted_frame_geometry",
            "u32::try_from(encrypted_size).map_err(|_| Error::FrameTooLarge)?",
            "encrypted_size as u32",
            "checked encrypted sender geometry encrypted_frame_geometry",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "data_frame_wire_len_from_payload_len_with_peer_key_bytes",
            "crate::peer::data_message_wire_len_from_payload_len::<RelayMessage<T>>(relay_len)",
            "relay_len",
            "checked P2P transport geometry "
            "data_frame_wire_len_from_payload_len_with_peer_key_bytes",
        ),
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "enqueue_encrypted",
            "if encrypted_size > "
            "self.max_frame_bytes.min(crate::MAX_ENCRYPTED_FRAME_BYTES) {",
            "if encrypted_size > self.max_frame_bytes {",
            "checked runtime-clamped encrypted geometry before cap/queue admission",
        ),
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "account_enqueued",
            "self.queued_safety_bytes = self\n"
            "                        .queued_safety_bytes\n"
            "                        .checked_add(frame_len)",
            "self.queued_safety_bytes = self\n"
            "                        .queued_safety_bytes\n"
            "                        .saturating_add(frame_len)",
            "checked admitted queue-byte accounting",
        ),
        (
            Path("crates/iroha_p2p/src/lib.rs"),
            "frame_plaintext_cap",
            ".min(MAX_ENCRYPTED_FRAME_BYTES)",
            ".min(usize::MAX)",
            "checked P2P transport geometry frame_plaintext_cap",
        ),
        (
            Path("crates/iroha_p2p/src/lib.rs"),
            "frame_queue_charge",
            ".checked_add(P2P_FRAME_LENGTH_PREFIX_BYTES)",
            ".checked_add(0)",
            "checked P2P transport geometry frame_queue_charge",
        ),
    ),
)
def test_transport_geometry_source_fidelity_rejects_short_p2p_frame_math(
    tmp_path: Path,
    relative: Path,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    assert module._transport_geometry_production_source_fidelity_errors(repo_root) == []

    mutate_rust_item_source(module, repo_root / relative, item_name, old, new)

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("item_name", "old", "new", "expected_error"),
    (
        (
            "checked_encoded_frame_len",
            "let encoded_len = ncore::encoded_frame_len(message)?;",
            "let encoded_len = 0;",
            "exact Norito counting preflight before P2P allocation",
        ),
        (
            "try_send",
            "if encrypted.len() > "
            "self.max_frame_bytes.min(crate::MAX_ENCRYPTED_FRAME_BYTES) {",
            "if false && encrypted.len() > "
            "self.max_frame_bytes.min(crate::MAX_ENCRYPTED_FRAME_BYTES) {",
            "QUIC counting preflight and post-encryption runtime-cap check",
        ),
        (
            "reserve_for_frame",
            "if size > self.max_frame_bytes.min(crate::MAX_ENCRYPTED_FRAME_BYTES) {",
            "if size > self.max_frame_bytes {",
            "runtime-clamped checked and incremental receiver reservation",
        ),
        (
            "reserve_for_frame",
            ".ok_or(Error::FrameTooLarge)?\n                .min(needed);",
            ".ok_or(Error::FrameTooLarge)?\n                .min(usize::MAX);",
            "runtime-clamped checked and incremental receiver reservation",
        ),
        (
            "prepare_message",
            "let encoded_len = "
            "checked_encoded_frame_len::<T, E>(msg, self.max_frame_bytes)?;",
            "let encoded_len = 0;",
            "counting sender preflight before material encoding",
        ),
        (
            "prepare_encoded_buffer",
            "let max_plaintext = frame_plaintext_cap_for::<E>(self.max_frame_bytes);",
            "let max_plaintext = usize::MAX;",
            "generic AEAD cap before sender batching",
        ),
        (
            "enqueue_encrypted",
            "if self.encrypted.len() != encrypted_size {",
            "if false && self.encrypted.len() != encrypted_size {",
            "post-encryption sender geometry agreement",
        ),
    ),
)
def test_transport_geometry_source_fidelity_rejects_runtime_frame_mutants(
    tmp_path: Path,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    peer_path = repo_root / "crates" / "iroha_p2p" / "src" / "peer.rs"
    mutate_rust_item_source(module, peer_path, item_name, old, new)

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("item_name", "old", "new", "expected_error"),
    (
        (
            "merge",
            "other.bytes = 0;",
            "let _released_on_drop = other.bytes;",
            "already-accounted source leases coalesce without release and reacquisition",
        ),
        (
            "credit_owner",
            "if required.len() > self.max_sources {",
            "if false && required.len() > self.max_sources {",
            "shared authenticated-source registry preserves identity, protected sources, and capacity",
        ),
    ),
)
def test_transport_geometry_source_fidelity_rejects_source_owner_mutants(
    tmp_path: Path,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    peer_path = repo_root / "crates" / "iroha_p2p" / "src" / "peer.rs"
    mutate_rust_item_source(module, peer_path, item_name, old, new)

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("item_name", "old", "new", "expected_error"),
    (
        (
            "try_reserve_for_source",
            "(Some(retained), Some(candidate)) => !retained.matches(candidate),",
            "(Some(_), Some(_)) => false,",
            "queued progress tickets must retain the exact weak delivery authority rather than reusing ordinal-equivalent tenure",
        ),
        (
            "try_reserve_for_source",
            "if source_retained.is_some_and(|retained| retained.items >= 1) {",
            "if source_retained.is_some_and(|retained| retained.items >= 2) {",
            "distinct broadcast or direct requests remain FIFO-ranked behind a target owner",
        ),
        (
            "submit_progress_message_to_source",
            "ProgressLeaseAttempt::SameRequestAlreadyOwned\n"
            "            | ProgressLeaseAttempt::CancelledMembership => return Ok(None),",
            "ProgressLeaseAttempt::SameRequestAlreadyOwned\n"
            "            | ProgressLeaseAttempt::CancelledMembership => "
            "return Ok(Some(NetworkActorAdmittedTicketIdentity::forged())),",
            (
                "same-request and cancelled admission return no new ticket identity, "
                "while invalid ownership cannot substitute for the original request"
            ),
        ),
        (
            "broadcast_recoverable",
            "&& Arc::ptr_eq(&ticket.topology, &self.reliable_broadcast_topology)",
            "&& true",
            "broadcast retry tickets bind digest, actor budget, and topology publication",
        ),
        (
            "broadcast_recoverable",
            "if !target.membership.is_active() {",
            "if false && !target.membership.is_active() {",
            "broadcast fanout admits each active topology authority through an isolated target source",
        ),
        (
            "progress_ticket_request_digest",
            "let metadata = [0_u8, priority_tag(post.priority)];",
            "let metadata = [1_u8, priority_tag(post.priority)];",
            "canonical progress digest keeps Post and Broadcast request identities disjoint",
        ),
    ),
)
def test_transport_geometry_source_fidelity_rejects_local_actor_split_mutants(
    tmp_path: Path,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    assert module._transport_geometry_production_source_fidelity_errors(repo_root) == []

    network_path = repo_root / "crates" / "iroha_p2p" / "src" / "network.rs"
    mutate_rust_item_source(module, network_path, item_name, old, new)

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(expected_error in error for error in errors), errors


def test_transport_geometry_rejects_ordinal_equivalent_weak_authority_mutant(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    network_path = repo_root / "crates" / "iroha_p2p" / "src" / "network.rs"
    mutate_rust_item_source_in_context(
        module,
        network_path,
        "matches",
        (("impl", "WeakProgressDeliveryAuthority"),),
        "Arc::ptr_eq(&retained, &candidate.tenure)",
        "retained.connection_ordinal == candidate.tenure.connection_ordinal",
    )

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(
        "weak progress authority matching must preserve exact Arc ownership"
        in error
        for error in errors
    ), errors


def _synthetic_production_trace_certificate(module) -> dict[str, object]:
    digest = "a" * 64
    symbol = {
        "path": "production.rs",
        "kind": "method",
        "symbol": "Runtime::authorize",
        "source_sha256": digest,
        "token_sha256": "b" * 64,
    }
    theorem = {
        "path": "proofs.rs",
        "kind": "verus_proof_fn",
        "symbol": "runtime_refines_next",
        "source_sha256": "c" * 64,
        "token_sha256": "d" * 64,
    }
    return {
        "schema_version": module.PRODUCTION_TRACE_EXTRACTION_EVIDENCE_SCHEMA_VERSION,
        "certificate_type": "production_trace_extraction_theorem",
        "theorem": module.PRODUCTION_TRACE_EXTRACTION_THEOREM,
        "canonical_encoding": module.PRODUCTION_TRACE_EXTRACTION_CANONICAL_ENCODING,
        "backend_verification": True,
        "workspace_source_manifest_sha256": "e" * 64,
        "formal_source_manifest_sha256": "f" * 64,
        "multilane_source_manifest_sha256": "1" * 64,
        "artifacts": [
            {"role": "proof_ledger", "sha256": "2" * 64, "size_bytes": 17}
        ],
        "model_sources": [
            {"path": "model.tla", "sha256": "3" * 64, "size_bytes": 31}
        ],
        "model_symbols": [
            {
                "path": "model.tla",
                "kind": "tla_operator",
                "symbol": "ApplyCarrier",
                "token_sha256": "4" * 64,
            }
        ],
        "refinement_symbols": [copy.deepcopy(symbol)],
        "production_symbols": [copy.deepcopy(symbol)],
        "verus_theorems": [copy.deepcopy(theorem)],
        "source_bindings": [
            {
                "id": "canonical_wsv_commit_authorization",
                "action_tags": ["APPLY_CARRIER"],
                "model_symbols": ["ApplyCarrier"],
                "production_symbol": copy.deepcopy(symbol),
                "authorization_source": None,
                "checked_transition_consumer": None,
                "checked_transition_adapter": None,
                "canonical_commit_sink": copy.deepcopy(symbol),
                "carrier_identity_projection": copy.deepcopy(symbol),
                "refinement_kernel": copy.deepcopy(symbol),
                "verus_theorem": copy.deepcopy(theorem),
                "authenticated": True,
            }
        ],
        "proof_linkage": {
            "ledger_document_sha256": "5" * 64,
            "tlaps_document_sha256": "6" * 64,
            "verus_document_sha256": "7" * 64,
            "cross_tool_document_sha256": "8" * 64,
            "cross_tool_ledger_sha256": "5" * 64,
            "cross_tool_component_evidence": {
                "tlaps_sha256": "6" * 64,
                "verus_sha256": "7" * 64,
            },
            "verus_log_sha256": "9" * 64,
            "machine_checked_completion": True,
        },
    }


def _synthetic_trace_artifact_paths(module, path: Path):
    return module.ProductionTraceExtractionArtifactPaths(
        ledger=path,
        evidence=path,
        verus_evidence=path,
        verus_log=path,
        cross_tool_evidence=path,
    )


def test_production_trace_certificate_authenticates_all_runtime_links() -> None:
    module = load_checker()
    snapshot = module._production_trace_extraction_source_snapshot()
    bindings = {
        binding["id"]: binding for binding in snapshot["source_bindings"]
    }
    assert set(bindings) == {
        "queue_plan_selection_and_reservation_fsync",
        "reservation_cleanup_prefixes",
        "pre_kura_direct_reservation_release",
        "producer_kura_activation",
        "execution_input_persistence",
        "durable_autonomous_bundle",
        "ready_qc_persistence",
        "lane_commit_persistence",
        "kura_slot_retirement_persistence",
        "kura_claim_release_prefixes",
        "queue_release_preparation_handoff",
        "queue_release_completion_publication",
        "ready_authorization",
        "ready_signature",
        "canonical_wsv_commit_authorization",
    }
    assert all(binding["authenticated"] is True for binding in bindings.values())
    shared_identities = {
        (
            binding["carrier_identity_projection"]["path"],
            binding["carrier_identity_projection"]["symbol"],
            binding["carrier_identity_projection"]["token_sha256"],
        )
        for binding in bindings.values()
    }
    assert len(shared_identities) == 1
    assert next(iter(shared_identities))[1] == (
        "canonical_lane_queue_reservation_group_identity_projection"
    )


def test_production_trace_certificate_keeps_unextracted_actions_explicit() -> None:
    module = load_checker()
    assert module.PRODUCTION_TRACE_EXTRACTION_OPEN_MODEL_ACTIONS == (
        "FanoutFromProducer",
        "ServeLateBody",
        "Crash",
        "Recover",
        "RecoverReservationSnapshot",
        "RepairPostCarrierEvidence",
    )

    with pytest.raises(ValueError, match="model actions remain unextracted"):
        module.build_production_trace_extraction_evidence(
            {"machine_checked_completion": True},
            tlaps_evidence={},
            verus_evidence={},
            cross_tool_evidence={},
            artifacts=None,
        )


def test_production_trace_certificate_rejects_erased_open_action_debt(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    module = load_checker()
    monkeypatch.setattr(module, "PRODUCTION_TRACE_EXTRACTION_OPEN_MODEL_ACTIONS", ())

    with pytest.raises(ValueError, match="do not partition the exact model-action"):
        module.build_production_trace_extraction_evidence(
            {"machine_checked_completion": True},
            tlaps_evidence={},
            verus_evidence={},
            cross_tool_evidence={},
            artifacts=None,
        )


def test_production_trace_certificate_rejects_model_action_inventory_drift(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    module = load_checker()
    monkeypatch.setattr(
        module,
        "PRODUCTION_TRACE_EXTRACTION_REQUIRED_MODEL_ACTIONS",
        module.PRODUCTION_TRACE_EXTRACTION_REQUIRED_MODEL_ACTIONS[:-1],
    )

    with pytest.raises(ValueError) as failure:
        module._production_trace_extraction_source_snapshot()

    assert (
        "required model actions differ from the multilane source-binding ledger"
        in str(failure.value)
    )


@pytest.mark.parametrize(
    ("binding_id", "action_tag"),
    (
        (
            "queue_plan_selection_and_reservation_fsync",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_SELECT_QUEUE_PLAN_V4",
        ),
        (
            "queue_plan_selection_and_reservation_fsync",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_FSYNC_RESERVATION_V5",
        ),
        (
            "reservation_cleanup_prefixes",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_RESERVATION_COMMITTED",
        ),
        (
            "pre_kura_direct_reservation_release",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_RELEASE_RESERVATION_DIRECT",
        ),
        (
            "producer_kura_activation",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_ACTIVATE_KURA",
        ),
        (
            "execution_input_persistence",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_EXECUTION_INPUT",
        ),
        (
            "durable_autonomous_bundle",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_EXECUTION_INPUT",
        ),
        (
            "ready_qc_persistence",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_READY_QC",
        ),
        (
            "lane_commit_persistence",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_LANE_COMMIT",
        ),
        (
            "kura_slot_retirement_persistence",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_KURA_RETIREMENT",
        ),
        (
            "kura_claim_release_prefixes",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_ADVANCE_RELEASE_PENDING",
        ),
        (
            "kura_claim_release_prefixes",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_ADVANCE_RELEASED",
        ),
        (
            "queue_release_preparation_handoff",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_PREPARE_RESERVATION_RELEASE",
        ),
        (
            "queue_release_completion_publication",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_COMPLETE_RESERVATION_RELEASE",
        ),
        (
            "queue_release_completion_publication",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_RESTORE_RELEASED_FIFO",
        ),
        (
            "queue_release_completion_publication",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_FORGET_RESERVATION_RELEASE",
        ),
        (
            "ready_authorization",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_AUTHORIZE_READY",
        ),
        (
            "ready_signature",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_SIGN_READY",
        ),
        (
            "canonical_wsv_commit_authorization",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_APPLY_CARRIER",
        ),
    ),
)
def test_production_trace_certificate_rejects_each_disconnected_runtime_link(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    binding_id: str,
    action_tag: str,
) -> None:
    module = load_checker()
    bindings = [
        copy.deepcopy(binding)
        for binding in module.PRODUCTION_TRACE_EXTRACTION_BINDINGS
    ]
    binding = next(candidate for candidate in bindings if candidate["id"] == binding_id)
    source_path = ROOT_DIR / binding["path"]
    source = source_path.read_text(encoding="utf-8")
    assert action_tag in source
    mutated_path = tmp_path / f"{binding_id}.rs"
    mutated_path.write_text(
        source.replace(action_tag, f"{action_tag}_MUTATED"),
        encoding="utf-8",
    )
    binding["path"] = str(mutated_path.resolve())
    monkeypatch.setattr(module, "PRODUCTION_TRACE_EXTRACTION_BINDINGS", tuple(bindings))

    with pytest.raises(ValueError) as failure:
        module._production_trace_extraction_source_snapshot()

    message = str(failure.value)
    assert binding_id in message
    assert "missing exact code tokens" in message


@pytest.mark.parametrize(
    "binding_id",
    (
        "queue_plan_selection_and_reservation_fsync",
        "reservation_cleanup_prefixes",
        "pre_kura_direct_reservation_release",
        "producer_kura_activation",
        "execution_input_persistence",
        "durable_autonomous_bundle",
        "ready_qc_persistence",
        "lane_commit_persistence",
        "kura_slot_retirement_persistence",
        "kura_claim_release_prefixes",
        "queue_release_preparation_handoff",
        "queue_release_completion_publication",
        "ready_authorization",
        "ready_signature",
        "canonical_wsv_commit_authorization",
    ),
)
def test_production_trace_certificate_rejects_each_disconnected_carrier_identity(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    binding_id: str,
) -> None:
    module = load_checker()
    bindings = [
        copy.deepcopy(binding)
        for binding in module.PRODUCTION_TRACE_EXTRACTION_BINDINGS
    ]
    binding = next(candidate for candidate in bindings if candidate["id"] == binding_id)
    helper = "canonical_lane_queue_reservation_group_identity_projection"
    endpoint = binding
    authority = binding.get("authorization_source")
    if authority is not None and helper in authority["required_tokens"]:
        endpoint = authority
    source_path = ROOT_DIR / endpoint["path"]
    source = source_path.read_text(encoding="utf-8")
    if helper not in source:
        endpoint = binding["authorization_source"]
        source_path = ROOT_DIR / endpoint["path"]
        source = source_path.read_text(encoding="utf-8")
    assert helper in source
    mutated_path = tmp_path / f"{binding_id}-identity.rs"
    mutated_path.write_text(
        source.replace(helper, f"{helper}_DISCONNECTED"),
        encoding="utf-8",
    )
    endpoint["path"] = str(mutated_path.resolve())
    monkeypatch.setattr(module, "PRODUCTION_TRACE_EXTRACTION_BINDINGS", tuple(bindings))

    with pytest.raises(ValueError) as failure:
        module._production_trace_extraction_source_snapshot()

    message = str(failure.value)
    assert binding_id in message
    assert helper in message


def test_production_trace_certificate_rejects_disconnected_selection_authority(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    module = load_checker()
    bindings = [
        copy.deepcopy(binding)
        for binding in module.PRODUCTION_TRACE_EXTRACTION_BINDINGS
    ]
    binding = next(
        candidate
        for candidate in bindings
        if candidate["id"] == "queue_plan_selection_and_reservation_fsync"
    )
    authority = binding["authorization_source"]
    source_path = ROOT_DIR / authority["path"]
    source = source_path.read_text(encoding="utf-8")
    required = "self.reservation_scope()"
    assert required in source
    mutated_path = tmp_path / "selection-authority-disconnected.rs"
    mutated_path.write_text(
        source.replace(required, "self.disconnected_selection_scope()"),
        encoding="utf-8",
    )
    authority["path"] = str(mutated_path.resolve())
    monkeypatch.setattr(module, "PRODUCTION_TRACE_EXTRACTION_BINDINGS", tuple(bindings))

    with pytest.raises(ValueError) as failure:
        module._production_trace_extraction_source_snapshot()

    message = str(failure.value)
    assert "missing canonical authorization source tokens" in message
    assert "reservation_scope" in message


def test_production_trace_certificate_rejects_disconnected_ready_signature_sink(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    module = load_checker()
    bindings = [
        copy.deepcopy(binding)
        for binding in module.PRODUCTION_TRACE_EXTRACTION_BINDINGS
    ]
    binding = next(
        candidate for candidate in bindings if candidate["id"] == "ready_signature"
    )
    sink = binding["commit_sink"]
    source_path = ROOT_DIR / sink["path"]
    source = source_path.read_text(encoding="utf-8")
    required = "authorization.consume_signing_request"
    assert required in source
    mutated_path = tmp_path / "ready-signature-sink-disconnected.rs"
    mutated_path.write_text(
        source.replace(required, "authorization.disconnected_signing_request"),
        encoding="utf-8",
    )
    sink["path"] = str(mutated_path.resolve())
    monkeypatch.setattr(module, "PRODUCTION_TRACE_EXTRACTION_BINDINGS", tuple(bindings))

    with pytest.raises(ValueError) as failure:
        module._production_trace_extraction_source_snapshot()

    message = str(failure.value)
    assert "missing canonical commit sink tokens" in message
    assert "consume_signing_request" in message


def test_production_trace_certificate_rejects_disconnected_apply_carrier_consumer(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    module = load_checker()
    bindings = [
        copy.deepcopy(binding)
        for binding in module.PRODUCTION_TRACE_EXTRACTION_BINDINGS
    ]
    binding = next(
        candidate
        for candidate in bindings
        if candidate["id"] == "canonical_wsv_commit_authorization"
    )
    consumer = binding["checked_transition_consumer"]
    source_path = ROOT_DIR / consumer["path"]
    source = source_path.read_text(encoding="utf-8")
    required = "checked.into_projection()"
    assert required in source
    mutated_path = tmp_path / "apply-carrier-consumer-disconnected.rs"
    mutated_path.write_text(
        source.replace(required, "checked.discard_without_projection()"),
        encoding="utf-8",
    )
    consumer["path"] = str(mutated_path.resolve())
    monkeypatch.setattr(module, "PRODUCTION_TRACE_EXTRACTION_BINDINGS", tuple(bindings))

    with pytest.raises(ValueError) as failure:
        module._production_trace_extraction_source_snapshot()

    message = str(failure.value)
    assert "missing move-only consumer canonical_wsv_commit_authorization" in message
    assert "checked.into_projection()" in message


@pytest.mark.parametrize(
    ("binding_token", "source_token", "replacement"),
    (
        (
            "CheckedCarrierApplications::for_block",
            "CheckedCarrierApplications::for_block",
            "CheckedCarrierApplications::disconnected_for_block",
        ),
        (
            "checked_carrier_applications.bind_execution_batch",
            ".bind_execution_batch(reference, execution_batch.lanes.len())",
            ".disconnected_execution_batch(reference, execution_batch.lanes.len())",
        ),
        (
            "checked_carrier_applications.push(checked, projection)",
            "checked_carrier_applications.push(checked, projection)",
            "checked_carrier_applications.discard(checked, projection)",
        ),
    ),
)
def test_production_trace_certificate_rejects_disconnected_apply_carrier_batch_binding(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    binding_token: str,
    source_token: str,
    replacement: str,
) -> None:
    module = load_checker()
    bindings = [
        copy.deepcopy(binding)
        for binding in module.PRODUCTION_TRACE_EXTRACTION_BINDINGS
    ]
    binding = next(
        candidate
        for candidate in bindings
        if candidate["id"] == "canonical_wsv_commit_authorization"
    )
    source_path = ROOT_DIR / binding["path"]
    source = source_path.read_text(encoding="utf-8")
    assert source_token in source
    mutated_path = tmp_path / "apply-carrier-batch-binding-disconnected.rs"
    mutated_path.write_text(source.replace(source_token, replacement), encoding="utf-8")
    binding["path"] = str(mutated_path.resolve())
    monkeypatch.setattr(module, "PRODUCTION_TRACE_EXTRACTION_BINDINGS", tuple(bindings))

    with pytest.raises(ValueError) as failure:
        module._production_trace_extraction_source_snapshot()

    message = str(failure.value)
    assert "missing authenticated binding canonical_wsv_commit_authorization" in message
    assert binding_token in message


def test_production_trace_certificate_rejects_disconnected_apply_carrier_adapter(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    module = load_checker()
    bindings = [
        copy.deepcopy(binding)
        for binding in module.PRODUCTION_TRACE_EXTRACTION_BINDINGS
    ]
    binding = next(
        candidate
        for candidate in bindings
        if candidate["id"] == "canonical_wsv_commit_authorization"
    )
    adapter = binding["checked_transition_adapter"]
    source_path = ROOT_DIR / adapter["path"]
    source = source_path.read_text(encoding="utf-8")
    required = "CheckedCarrierApplications::consume_for_state_commit"
    assert required in source
    mutated_path = tmp_path / "apply-carrier-adapter-disconnected.rs"
    mutated_path.write_text(
        source.replace(
            required,
            "CheckedCarrierApplications::discard_before_state_commit",
        ),
        encoding="utf-8",
    )
    adapter["path"] = str(mutated_path.resolve())
    monkeypatch.setattr(module, "PRODUCTION_TRACE_EXTRACTION_BINDINGS", tuple(bindings))

    with pytest.raises(ValueError) as failure:
        module._production_trace_extraction_source_snapshot()

    message = str(failure.value)
    assert "missing move-only State commit adapter" in message
    assert required in message


def test_production_trace_certificate_rejects_disconnected_apply_carrier_forwarder(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    module = load_checker()
    bindings = [
        copy.deepcopy(binding)
        for binding in module.PRODUCTION_TRACE_EXTRACTION_BINDINGS
    ]
    binding = next(
        candidate
        for candidate in bindings
        if candidate["id"] == "canonical_wsv_commit_authorization"
    )
    source_binding = binding["authorization_source"]
    source_path = ROOT_DIR / source_binding["path"]
    source = source_path.read_text(encoding="utf-8")
    required = "Box::new(checked_carrier_applications)"
    assert required in source
    mutated_path = tmp_path / "apply-carrier-forwarder-disconnected.rs"
    mutated_path.write_text(
        source.replace(required, "Box::new(disconnected_carrier_applications)"),
        encoding="utf-8",
    )
    source_binding["path"] = str(mutated_path.resolve())
    monkeypatch.setattr(module, "PRODUCTION_TRACE_EXTRACTION_BINDINGS", tuple(bindings))

    with pytest.raises(ValueError) as failure:
        module._production_trace_extraction_source_snapshot()

    message = str(failure.value)
    assert "missing canonical authorization source tokens" in message
    assert required in message


def test_production_trace_certificate_rejects_disconnected_state_commit_sink(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    module = load_checker()
    bindings = [
        copy.deepcopy(binding)
        for binding in module.PRODUCTION_TRACE_EXTRACTION_BINDINGS
    ]
    binding = next(
        candidate
        for candidate in bindings
        if candidate["id"] == "canonical_wsv_commit_authorization"
    )
    sink = binding["commit_sink"]
    source_path = ROOT_DIR / sink["path"]
    source = source_path.read_text(encoding="utf-8")
    required = ".consume_for_state_commit(block_header_hash, staged_merge_entry.as_ref())"
    assert required in source
    mutated_path = tmp_path / "state-commit-sink-disconnected.rs"
    mutated_path.write_text(
        source.replace(
            required,
            ".discard_before_state_commit(block_header_hash, staged_merge_entry.as_ref())",
        ),
        encoding="utf-8",
    )
    sink["path"] = str(mutated_path.resolve())
    monkeypatch.setattr(module, "PRODUCTION_TRACE_EXTRACTION_BINDINGS", tuple(bindings))

    with pytest.raises(ValueError) as failure:
        module._production_trace_extraction_source_snapshot()

    message = str(failure.value)
    assert "missing canonical commit sink tokens" in message
    assert "authorization.consume_for_state_commit" in message


def test_production_trace_certificate_rejects_apply_carrier_after_state_commit(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    module = load_checker()
    bindings = [
        copy.deepcopy(binding)
        for binding in module.PRODUCTION_TRACE_EXTRACTION_BINDINGS
    ]
    binding = next(
        candidate
        for candidate in bindings
        if candidate["id"] == "canonical_wsv_commit_authorization"
    )
    sink = binding["commit_sink"]
    source_path = ROOT_DIR / sink["path"]
    source = source_path.read_text(encoding="utf-8")
    authorization_start = source.index(
        "        if tx_validate_accepted && !replay_prevalidation {\n"
        "            match state_commit_authorization.take()"
    )
    authorization_end = source.index(
        "        let autoscale_storage_hold", authorization_start
    )
    authorization_block = source[authorization_start:authorization_end]
    without_authorization = (
        source[:authorization_start] + source[authorization_end:]
    )
    transaction_commit = "            let tx_commit_result = transactions.commit();"
    insertion = without_authorization.index(transaction_commit) + len(transaction_commit)
    mutated = (
        without_authorization[:insertion]
        + "\n"
        + authorization_block
        + without_authorization[insertion:]
    )
    mutated_path = tmp_path / "apply-carrier-after-state-commit.rs"
    mutated_path.write_text(mutated, encoding="utf-8")
    sink["path"] = str(mutated_path.resolve())
    monkeypatch.setattr(module, "PRODUCTION_TRACE_EXTRACTION_BINDINGS", tuple(bindings))

    with pytest.raises(ValueError) as failure:
        module._production_trace_extraction_source_snapshot()

    message = str(failure.value)
    assert "missing canonical commit sink tokens" in message
    assert "moved before its predecessor" in message


@pytest.mark.parametrize(
    ("edge", "required", "replacement", "expected_message"),
    (
        (
            "authorization_source",
            "lane_queue_reservation_group_binding_from_ordered_keys",
            "disconnected_lane_queue_reservation_group_binding",
            "missing canonical authorization source tokens",
        ),
        (
            "commit_sink",
            "write_autonomous_lane_block_view_state_record_locked",
            "disconnect_autonomous_lane_block_view_state_record_locked",
            "missing canonical commit sink tokens",
        ),
    ),
)
def test_production_trace_certificate_rejects_disconnected_ready_qc_edges(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    edge: str,
    required: str,
    replacement: str,
    expected_message: str,
) -> None:
    module = load_checker()
    bindings = [
        copy.deepcopy(binding)
        for binding in module.PRODUCTION_TRACE_EXTRACTION_BINDINGS
    ]
    binding = next(
        candidate for candidate in bindings if candidate["id"] == "ready_qc_persistence"
    )
    endpoint = binding[edge]
    source_path = ROOT_DIR / endpoint["path"]
    source = source_path.read_text(encoding="utf-8")
    assert required in source
    mutated_path = tmp_path / f"ready-qc-{edge}-disconnected.rs"
    mutated_path.write_text(source.replace(required, replacement), encoding="utf-8")
    endpoint["path"] = str(mutated_path.resolve())
    monkeypatch.setattr(module, "PRODUCTION_TRACE_EXTRACTION_BINDINGS", tuple(bindings))

    with pytest.raises(ValueError) as failure:
        module._production_trace_extraction_source_snapshot()

    message = str(failure.value)
    assert expected_message in message
    assert required.split("(", maxsplit=1)[0] in message


@pytest.mark.parametrize(
    ("edge", "required", "replacement", "expected_message"),
    (
        (
            "authorization_source",
            "lane_queue_reservation_group_binding_from_ordered_keys",
            "disconnected_lane_queue_reservation_group_binding",
            "missing canonical authorization source tokens",
        ),
        (
            "commit_sink",
            "sync_indexed_sidecar_initial_data",
            "disconnect_indexed_sidecar_initial_data",
            "missing canonical commit sink tokens",
        ),
    ),
)
def test_production_trace_certificate_rejects_disconnected_execution_input_edges(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    edge: str,
    required: str,
    replacement: str,
    expected_message: str,
) -> None:
    module = load_checker()
    bindings = [
        copy.deepcopy(binding)
        for binding in module.PRODUCTION_TRACE_EXTRACTION_BINDINGS
    ]
    binding = next(
        candidate for candidate in bindings if candidate["id"] == "execution_input_persistence"
    )
    endpoint = binding[edge]
    source_path = ROOT_DIR / endpoint["path"]
    source = source_path.read_text(encoding="utf-8")
    assert required in source
    mutated_path = tmp_path / f"execution-input-{edge}-disconnected.rs"
    mutated_path.write_text(source.replace(required, replacement), encoding="utf-8")
    endpoint["path"] = str(mutated_path.resolve())
    monkeypatch.setattr(module, "PRODUCTION_TRACE_EXTRACTION_BINDINGS", tuple(bindings))

    with pytest.raises(ValueError) as failure:
        module._production_trace_extraction_source_snapshot()

    message = str(failure.value)
    assert expected_message in message
    assert required.split("(", maxsplit=1)[0] in message


@pytest.mark.parametrize(
    ("edge", "required", "replacement", "expected_message"),
    (
        (
            "authorization_source",
            "lane_queue_reservation_group_binding_from_ordered_keys",
            "disconnected_lane_queue_reservation_group_binding",
            "missing canonical authorization source tokens",
        ),
        (
            "commit_sink",
            "promote_bound_progress_temp",
            "disconnect_bound_progress_temp",
            "missing canonical commit sink tokens",
        ),
    ),
)
def test_production_trace_certificate_rejects_disconnected_lane_commit_edges(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    edge: str,
    required: str,
    replacement: str,
    expected_message: str,
) -> None:
    module = load_checker()
    bindings = [
        copy.deepcopy(binding)
        for binding in module.PRODUCTION_TRACE_EXTRACTION_BINDINGS
    ]
    binding = next(
        candidate for candidate in bindings if candidate["id"] == "lane_commit_persistence"
    )
    endpoint = binding[edge]
    source_path = ROOT_DIR / endpoint["path"]
    source = source_path.read_text(encoding="utf-8")
    assert required in source
    mutated_path = tmp_path / f"lane-commit-{edge}-disconnected.rs"
    mutated_path.write_text(source.replace(required, replacement), encoding="utf-8")
    endpoint["path"] = str(mutated_path.resolve())
    monkeypatch.setattr(module, "PRODUCTION_TRACE_EXTRACTION_BINDINGS", tuple(bindings))

    with pytest.raises(ValueError) as failure:
        module._production_trace_extraction_source_snapshot()

    message = str(failure.value)
    assert expected_message in message
    assert required.split("(", maxsplit=1)[0] in message


@pytest.mark.parametrize(
    ("binding_id", "edge", "required", "replacement", "expected_message"),
    (
        (
            "kura_slot_retirement_persistence",
            "authorization_source",
            "canonical_lane_queue_reservation_group_identity_projection",
            "disconnected_lane_queue_reservation_group_identity_projection",
            "missing canonical authorization source tokens",
        ),
        (
            "kura_slot_retirement_persistence",
            "commit_sink",
            "write_atomic_synced_replace",
            "disconnected_atomic_synced_replace",
            "missing canonical commit sink tokens",
        ),
        (
            "kura_claim_release_prefixes",
            "authorization_source",
            "replacement.retirement_hash()",
            "replacement.disconnected_retirement_hash()",
            "missing canonical authorization source tokens",
        ),
        (
            "kura_claim_release_prefixes",
            "commit_sink",
            "persist(path)",
            "disconnected_persist(path)",
            "missing canonical commit sink tokens",
        ),
        (
            "queue_release_preparation_handoff",
            "authorization_source",
            "autonomous_lane_entrypoint_claim_release_progress_locked",
            "disconnected_lane_entrypoint_claim_release_progress_locked",
            "missing canonical authorization source tokens",
        ),
        (
            "queue_release_preparation_handoff",
            "commit_sink",
            "journal.prepare_release(barrier.clone())",
            "journal.disconnected_prepare_release(barrier.clone())",
            "missing canonical commit sink tokens",
        ),
        (
            "queue_release_completion_publication",
            "authorization_source",
            "consume_for_claim_transition",
            "disconnected_claim_transition",
            "missing canonical authorization source tokens",
        ),
        (
            "queue_release_completion_publication",
            "primary",
            "release_barrier_has_exact_fifo_ownership_locked",
            "disconnected_release_barrier_fifo_ownership",
            "missing authenticated binding",
        ),
        (
            "pre_kura_direct_reservation_release",
            "primary",
            "journal.release_batch",
            "journal.disconnected_release_batch",
            "missing authenticated binding",
        ),
    ),
)
def test_production_trace_certificate_rejects_disconnected_kura_release_edges(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    binding_id: str,
    edge: str,
    required: str,
    replacement: str,
    expected_message: str,
) -> None:
    module = load_checker()
    bindings = [
        copy.deepcopy(binding)
        for binding in module.PRODUCTION_TRACE_EXTRACTION_BINDINGS
    ]
    binding = next(candidate for candidate in bindings if candidate["id"] == binding_id)
    endpoint = binding if edge == "primary" else binding[edge]
    source_path = ROOT_DIR / endpoint["path"]
    source = source_path.read_text(encoding="utf-8")
    assert required in source
    mutated_path = tmp_path / f"{binding_id}-{edge}-disconnected.rs"
    mutated_path.write_text(source.replace(required, replacement), encoding="utf-8")
    endpoint["path"] = str(mutated_path.resolve())
    monkeypatch.setattr(module, "PRODUCTION_TRACE_EXTRACTION_BINDINGS", tuple(bindings))

    with pytest.raises(ValueError) as failure:
        module._production_trace_extraction_source_snapshot()

    message = str(failure.value)
    assert expected_message in message
    assert required.split("(", maxsplit=1)[0] in message


@pytest.mark.parametrize(
    ("edge", "required", "replacement", "expected_message"),
    (
        (
            "authorization_source",
            "authorization.height_context_id()",
            "authorization.disconnected_height_context_id()",
            "missing canonical authorization source tokens",
        ),
        (
            "commit_sink",
            "finalize_autonomous_lane_entrypoint_claims_locked",
            "disconnect_autonomous_lane_entrypoint_claims_locked",
            "missing canonical commit sink tokens",
        ),
    ),
)
def test_production_trace_certificate_rejects_disconnected_kura_activation_edges(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    edge: str,
    required: str,
    replacement: str,
    expected_message: str,
) -> None:
    module = load_checker()
    bindings = [
        copy.deepcopy(binding)
        for binding in module.PRODUCTION_TRACE_EXTRACTION_BINDINGS
    ]
    binding = next(
        candidate for candidate in bindings if candidate["id"] == "producer_kura_activation"
    )
    endpoint = binding[edge]
    source_path = ROOT_DIR / endpoint["path"]
    source = source_path.read_text(encoding="utf-8")
    assert required in source
    mutated_path = tmp_path / f"producer-kura-{edge}-disconnected.rs"
    mutated_path.write_text(source.replace(required, replacement), encoding="utf-8")
    endpoint["path"] = str(mutated_path.resolve())
    monkeypatch.setattr(module, "PRODUCTION_TRACE_EXTRACTION_BINDINGS", tuple(bindings))

    with pytest.raises(ValueError) as failure:
        module._production_trace_extraction_source_snapshot()

    message = str(failure.value)
    assert expected_message in message
    assert required.split("(", maxsplit=1)[0] in message


def test_production_trace_certificate_writer_emits_one_canonical_encoding(
    tmp_path: Path,
) -> None:
    module = load_checker()
    certificate = _synthetic_production_trace_certificate(module)
    path = tmp_path / "production_trace_extraction_evidence.json"

    module.write_production_trace_extraction_evidence(path, certificate)

    assert module.load_production_trace_extraction_evidence(path) == certificate
    assert path.read_bytes() == module._production_trace_canonical_json_bytes(
        certificate
    )


def test_production_trace_certificate_rejects_duplicate_json_keys(
    tmp_path: Path,
) -> None:
    module = load_checker()
    path = tmp_path / "duplicate.json"
    path.write_text('{"schema_version":1,"schema_version":1}\n', encoding="utf-8")

    with pytest.raises(ValueError, match="duplicate JSON key"):
        module.load_production_trace_extraction_evidence(path)


def test_production_trace_certificate_rejects_malformed_and_noncanonical_json(
    tmp_path: Path,
) -> None:
    module = load_checker()
    malformed = tmp_path / "malformed.json"
    malformed.write_text('{"schema_version":\n', encoding="utf-8")
    noncanonical = tmp_path / "noncanonical.json"
    noncanonical.write_text('{ "schema_version": 1 }\n', encoding="utf-8")

    with pytest.raises(ValueError, match="invalid JSON"):
        module.load_production_trace_extraction_evidence(malformed)
    with pytest.raises(ValueError, match="not canonical"):
        module.load_production_trace_extraction_evidence(noncanonical)


def test_production_trace_certificate_rejects_oversize_and_symlink_inputs(
    tmp_path: Path,
) -> None:
    module = load_checker()
    oversized = tmp_path / "oversized.json"
    oversized.write_bytes(
        b"{" + b" " * module.PRODUCTION_TRACE_EXTRACTION_EVIDENCE_MAX_BYTES + b"}\n"
    )
    target = tmp_path / "target.json"
    target.write_text("{}\n", encoding="utf-8")
    alias = tmp_path / "alias.json"
    alias.symlink_to(target)

    with pytest.raises(ValueError, match="at most"):
        module.load_production_trace_extraction_evidence(oversized)
    with pytest.raises(ValueError, match="non-symlink"):
        module.load_production_trace_extraction_evidence(alias)
    with pytest.raises(ValueError, match="non-symlink"):
        module._production_trace_artifact_entry("proof_ledger", alias)


def test_production_trace_certificate_rejects_hardlink_inputs_and_output(
    tmp_path: Path,
) -> None:
    module = load_checker()
    certificate = _synthetic_production_trace_certificate(module)
    original = tmp_path / "original.json"
    original.write_bytes(module._production_trace_canonical_json_bytes(certificate))
    input_alias = tmp_path / "input-hardlink.json"
    output_alias = tmp_path / "output-hardlink.json"
    try:
        os.link(original, input_alias)
        os.link(original, output_alias)
    except OSError as error:
        pytest.skip(f"hard links unavailable: {error}")

    with pytest.raises(ValueError, match="exactly one hard link"):
        module.load_production_trace_extraction_evidence(input_alias)
    with pytest.raises(ValueError, match="exactly one hard link"):
        module._production_trace_artifact_entry("proof_ledger", input_alias)
    with pytest.raises(ValueError, match="exactly one hard link"):
        module.write_production_trace_extraction_evidence(
            output_alias, certificate
        )
    assert output_alias.samefile(original)


def test_production_trace_certificate_rejects_symlinked_parent_components(
    tmp_path: Path,
) -> None:
    module = load_checker()
    certificate = _synthetic_production_trace_certificate(module)
    real_parent = tmp_path / "real"
    real_parent.mkdir()
    real_certificate = real_parent / "certificate.json"
    real_certificate.write_bytes(
        module._production_trace_canonical_json_bytes(certificate)
    )
    linked_parent = tmp_path / "linked-parent"
    try:
        linked_parent.symlink_to(real_parent, target_is_directory=True)
    except OSError as error:
        pytest.skip(f"symlinks unavailable: {error}")

    with pytest.raises(ValueError, match="parent path contains a symlink component"):
        module.load_production_trace_extraction_evidence(
            linked_parent / real_certificate.name
        )
    with pytest.raises(ValueError, match="parent path contains a symlink component"):
        module.write_production_trace_extraction_evidence(
            linked_parent / "output.json", certificate
        )


def test_production_trace_certificate_rejects_every_top_level_field_drift(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = load_checker()
    expected = _synthetic_production_trace_certificate(module)
    artifact = tmp_path / "artifact"
    artifact.write_bytes(b"evidence\n")
    paths = _synthetic_trace_artifact_paths(module, artifact)
    monkeypatch.setattr(
        module,
        "build_production_trace_extraction_evidence",
        lambda *args, **kwargs: expected,
    )

    for field in expected:
        observed = copy.deepcopy(expected)
        del observed[field]
        errors = module._production_trace_extraction_evidence_errors(
            {},
            observed,
            tlaps_evidence={},
            verus_evidence={},
            cross_tool_evidence={},
            artifacts=paths,
        )
        assert errors and "canonical current theorem certificate" in errors[0], field


def test_production_trace_certificate_rejects_every_nested_field_hash_and_source_drift(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = load_checker()
    expected = _synthetic_production_trace_certificate(module)
    artifact = tmp_path / "artifact"
    artifact.write_bytes(b"evidence\n")
    paths = _synthetic_trace_artifact_paths(module, artifact)
    monkeypatch.setattr(
        module,
        "build_production_trace_extraction_evidence",
        lambda *args, **kwargs: expected,
    )

    def leaf_paths(value, prefix=()):
        if isinstance(value, dict):
            for key in sorted(value):
                yield from leaf_paths(value[key], (*prefix, key))
        elif isinstance(value, list):
            for index, item in enumerate(value):
                yield from leaf_paths(item, (*prefix, index))
        else:
            yield prefix

    for path in leaf_paths(expected):
        observed = copy.deepcopy(expected)
        owner = observed
        for component in path[:-1]:
            owner = owner[component]
        original = owner[path[-1]]
        if isinstance(original, bool):
            replacement = not original
        elif isinstance(original, int):
            replacement = original + 1
        else:
            replacement = f"{original}-drift"
        owner[path[-1]] = replacement
        errors = module._production_trace_extraction_evidence_errors(
            {},
            observed,
            tlaps_evidence={},
            verus_evidence={},
            cross_tool_evidence={},
            artifacts=paths,
        )
        assert errors and "canonical current theorem certificate" in errors[0], path


def test_production_trace_certificate_rejects_missing_proof_linkage(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = load_checker()
    expected = _synthetic_production_trace_certificate(module)
    artifact = tmp_path / "artifact"
    artifact.write_bytes(b"evidence\n")
    paths = _synthetic_trace_artifact_paths(module, artifact)
    monkeypatch.setattr(
        module,
        "build_production_trace_extraction_evidence",
        lambda *args, **kwargs: expected,
    )
    observed = copy.deepcopy(expected)
    del observed["proof_linkage"]["cross_tool_component_evidence"]

    errors = module._production_trace_extraction_evidence_errors(
        {},
        observed,
        tlaps_evidence={},
        verus_evidence={},
        cross_tool_evidence={},
        artifacts=paths,
    )

    assert errors == [
        "production trace-extraction evidence does not match the canonical "
        "current theorem certificate at $.proof_linkage"
    ]
