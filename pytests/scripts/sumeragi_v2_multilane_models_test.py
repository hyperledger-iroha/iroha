"""Negative controls for the multilane model/source-binding contract."""

from __future__ import annotations

import copy
import importlib.util
import json
import shutil
import sys
from pathlib import Path

import pytest


ROOT_DIR = Path(__file__).resolve().parents[2]
CHECKER = (
    ROOT_DIR / "scripts" / "formal" / "check_sumeragi_v2_multilane_models.py"
)
BINDINGS = (
    ROOT_DIR
    / "formal"
    / "sumeragi_v2"
    / "multilane_source_bindings.json"
)


def load_checker():
    spec = importlib.util.spec_from_file_location(
        "sumeragi_v2_multilane_models", CHECKER
    )
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def canonical_contract() -> dict:
    ledger = json.loads(BINDINGS.read_text(encoding="utf-8"))
    return copy.deepcopy(ledger["inflight_first_release_layout_contract"])


def canonical_models() -> list[dict]:
    ledger = json.loads(BINDINGS.read_text(encoding="utf-8"))
    return copy.deepcopy(ledger["models"])


def canonical_queue_plan_model() -> dict:
    """Return the reviewed QueuePlan admission-registry source contract."""

    matches = [
        model
        for model in canonical_models()
        if model["module"] == "SumeragiV2QueuePlanAdmissionRegistry"
    ]
    assert len(matches) == 1
    return matches[0]


def copy_queue_plan_model_fixture(
    tmp_path: Path, module, model: dict
) -> Path:
    """Copy the QueuePlan model and every bound production source."""

    formal_dir = tmp_path / module.FORMAL_RELATIVE
    relatives = {
        module.FORMAL_RELATIVE / f"{model['module']}.tla",
        module.FORMAL_RELATIVE / model["positive_config"],
    }
    relatives.update(
        module.FORMAL_RELATIVE / mutation["config"]
        for mutation in model["mutations"]
    )
    relatives.update(
        Path(binding["path"]) for binding in model["production_symbols"]
    )
    relatives.update(
        Path(relative)
        for relative, _symbol, _tokens in module.QUEUE_PLAN_STARTUP_REPLAY_TEST_BINDINGS
    )
    for relative in relatives:
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT_DIR / relative, destination)
    return formal_dir


def validate_queue_plan_model_fixture(
    tmp_path: Path, module, model: dict, formal_dir: Path
) -> tuple[str, ...]:
    errors: list[str] = []
    module._validate_model(tmp_path, formal_dir, model, errors)
    module._validate_queue_plan_startup_replay_contract(
        tmp_path, [model], errors
    )
    return tuple(errors)


def test_queue_plan_future_source_bindings_accept_current_production(
    tmp_path: Path,
) -> None:
    module = load_checker()
    model = canonical_queue_plan_model()
    formal_dir = copy_queue_plan_model_fixture(tmp_path, module, model)
    assert validate_queue_plan_model_fixture(
        tmp_path, module, model, formal_dir
    ) == ()


@pytest.mark.parametrize(
    ("relative", "region", "old", "new", "symbol", "required_token"),
    (
        (
            Path("crates/iroha_core/src/sumeragi/v2_lane_work.rs"),
            "fn accept_queue_plan_admission_certificate(",
            "PendingQueuePlanAdmissionDisposition::Future => {",
            "PendingQueuePlanAdmissionDisposition::Stale => {",
            "V2LaneWorkAdapter::accept_queue_plan_admission_certificate",
            "PendingQueuePlanAdmissionDisposition::Future",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/v2_lane_work.rs"),
            "fn refresh_merge_candidates(",
            "PendingQueuePlanAdmissionDisposition::Future => {",
            "PendingQueuePlanAdmissionDisposition::EligibleAbsent => {",
            "refresh_merge_candidates",
            "PendingQueuePlanAdmissionDisposition::Future",
        ),
        (
            Path("crates/iroha_torii/src/lib.rs"),
            "fn validate_queue_plan_admission_publication(",
            "PendingQueuePlanAdmissionDisposition::Future => {",
            "PendingQueuePlanAdmissionDisposition::Stale => {",
            "validate_queue_plan_admission_publication",
            "PendingQueuePlanAdmissionDisposition::Future",
        ),
        (
            Path("crates/iroha_torii/src/lib.rs"),
            "fn ingest_queue_plan_admission_publication(",
            "validate_queue_plan_admission_publication(app, publication)?",
            "validate_queue_plan_admission_publication(app, publication).unwrap()",
            "ingest_queue_plan_admission_publication",
            "validate_queue_plan_admission_publication(app, publication)?",
        ),
    ),
    ids=(
        "sumeragi-ingress-rejects-future",
        "durable-recovery-retains-future",
        "torii-ingress-rejects-future",
        "torii-validates-before-persisting",
    ),
)
def test_queue_plan_future_source_bindings_reject_mutations(
    tmp_path: Path,
    relative: Path,
    region: str,
    old: str,
    new: str,
    symbol: str,
    required_token: str,
) -> None:
    module = load_checker()
    model = canonical_queue_plan_model()
    formal_dir = copy_queue_plan_model_fixture(tmp_path, module, model)
    path = tmp_path / relative
    source = path.read_text(encoding="utf-8")
    region_start = source.index(region)
    mutation = source.index(old, region_start)
    path.write_text(
        source[:mutation] + new + source[mutation + len(old) :],
        encoding="utf-8",
    )
    errors = validate_queue_plan_model_fixture(
        tmp_path, module, model, formal_dir
    )
    assert any(
        symbol in error and repr(required_token) in error for error in errors
    ), errors


@pytest.mark.parametrize(
    "symbol",
    tuple(
        binding[2]
        for binding in (
            (
                "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
                "method",
                "V2LaneWorkAdapter::accept_queue_plan_admission_certificate",
            ),
            (
                "crates/iroha_torii/src/lib.rs",
                "fn",
                "validate_queue_plan_admission_publication",
            ),
            (
                "crates/iroha_torii/src/lib.rs",
                "fn",
                "ingest_queue_plan_admission_publication",
            ),
            (
                "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
                "fn",
                "refresh_merge_candidates",
            ),
        )
    ),
)
def test_queue_plan_future_source_bindings_require_every_reviewed_member(
    tmp_path: Path, symbol: str
) -> None:
    module = load_checker()
    model = canonical_queue_plan_model()
    formal_dir = copy_queue_plan_model_fixture(tmp_path, module, model)
    model["production_symbols"] = [
        binding
        for binding in model["production_symbols"]
        if binding["symbol"] != symbol
    ]
    errors = validate_queue_plan_model_fixture(
        tmp_path, module, model, formal_dir
    )
    assert any(
        "reviewed startup replay binding" in error
        and symbol in error
        and "found 0" in error
        for error in errors
    ), errors


def copy_layout_fixture(tmp_path: Path, module, contract: dict) -> None:
    """Copy every file consumed by the isolated layout-contract validator."""

    relatives = {
        module.FORMAL_RELATIVE / f"{contract['module']}.tla",
        module.FORMAL_RELATIVE / contract["positive_config"],
        Path(contract["runner"]),
        Path(contract["evidence"]),
        module.CLOSURE_LEDGER_RELATIVE,
        module.INFLIGHT_LAYOUT_TEST,
    }
    relatives.update(
        module.FORMAL_RELATIVE / mutation["config"]
        for mutation in contract["mutations"]
    )
    relatives.update(
        Path(binding["path"]) for binding in contract["production_symbols"]
    )
    relatives.update(
        Path(check["path"]) for check in contract["ordered_source_checks"]
    )
    relatives.update(
        Path(check["path"]) for check in contract["forbidden_source_checks"]
    )
    relatives.update(Path(check["path"]) for check in contract["source_checks"])
    for relative in relatives:
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT_DIR / relative, destination)


def validate_fixture(tmp_path: Path, module, contract: dict) -> tuple[str, ...]:
    errors: list[str] = []
    module._validate_inflight_layout_contract(
        tmp_path,
        tmp_path / module.FORMAL_RELATIVE,
        contract,
        errors,
    )
    return tuple(errors)


def replace_once(path: Path, old: str, new: str) -> None:
    source = path.read_text(encoding="utf-8")
    assert source.count(old) >= 1, f"fixture cannot find {old!r} in {path}"
    path.write_text(source.replace(old, new, 1), encoding="utf-8")


def swap_ordered_once(path: Path, earlier: str, later: str) -> None:
    """Swap one ordered token pair while retaining both source anchors."""

    source = path.read_text(encoding="utf-8")
    earlier_offset = source.find(earlier)
    assert earlier_offset >= 0, f"fixture cannot find {earlier!r} in {path}"
    later_offset = source.find(later, earlier_offset + len(earlier))
    assert later_offset >= 0, (
        f"fixture cannot find {later!r} after {earlier!r} in {path}"
    )
    middle = source[earlier_offset + len(earlier) : later_offset]
    path.write_text(
        source[:earlier_offset]
        + later
        + middle
        + earlier
        + source[later_offset + len(later) :],
        encoding="utf-8",
    )


def copy_native_prepublication_fixture(
    tmp_path: Path, module
) -> list[dict]:
    """Copy the production files consumed by the ML-NAT-06 order contract."""

    models = canonical_models()
    relatives = {
        Path(relative)
        for relative, _, _, _ in module.NATIVE_PREPUBLICATION_BINDINGS
    }
    for relative in relatives:
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT_DIR / relative, destination)
    return models


def validate_native_prepublication_fixture(
    tmp_path: Path, module, models: list[dict]
) -> tuple[str, ...]:
    errors: list[str] = []
    module._validate_native_prepublication_contract(tmp_path, models, errors)
    return tuple(errors)


def test_native_prepublication_contract_accepts_current_production(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_native_prepublication_fixture(tmp_path, module)
    assert validate_native_prepublication_fixture(tmp_path, module, models) == ()


def test_native_prepublication_contract_rejects_removed_prepublication_call(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_native_prepublication_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_apply.rs"
    replace_once(
        path,
        ".prepublish_native_amx_participant_application_evidence(",
        ".skip_native_amx_participant_application_evidence(",
    )
    errors = validate_native_prepublication_fixture(tmp_path, module, models)
    assert any(
        "V2ApplyService::validate_and_apply" in error
        and "prepublish_native_amx_participant_application_evidence" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("earlier", "later"),
    (
        (
            ".store_v2_finality_artifact(artifact)",
            ".prepublish_native_amx_participant_application_evidence(",
        ),
        (
            ".prepublish_native_amx_participant_application_evidence(",
            "State::native_amx_participant_frontier_markers(",
        ),
        (
            "State::native_amx_participant_frontier_markers(",
            "token.authenticates_state_frontiers(",
        ),
        (
            "token.authenticates_state_frontiers(",
            ".apply_without_execution_with_verified_v2_finality("
            "&committed_block, commit_topology)",
        ),
        (
            ".apply_without_execution_with_verified_v2_finality("
            "&committed_block, commit_topology)",
            "state_block.commit().map_err",
        ),
    ),
    ids=(
        "finality-before-prepublication",
        "prepublication-before-state-frontier-projection",
        "state-frontier-projection-before-readback-token",
        "state-bound-readback-token-before-wsv-stage",
        "wsv-stage-before-wsv-commit",
    ),
)
def test_native_prepublication_contract_rejects_apply_order_drift(
    tmp_path: Path, earlier: str, later: str
) -> None:
    module = load_checker()
    models = copy_native_prepublication_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_apply.rs"
    swap_ordered_once(path, earlier, later)
    errors = validate_native_prepublication_fixture(tmp_path, module, models)
    assert any(
        "ordered Native prepublication item "
        "V2ApplyService::validate_and_apply" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("earlier", "later"),
    (
        (
            "self.write_native_amx_participant_application_manifest_artifact_"
            "with_retention_policy_under_publication_guard(",
            "self.write_native_amx_participant_application_receipt_artifact_"
            "only_with_retention_policy_under_publication_guard(",
        ),
        (
            "self.write_native_amx_participant_application_manifest_artifact_"
            "with_retention_policy_under_publication_guard(",
            "self.read_back_native_amx_plan_manifests_under_publication_guard(",
        ),
        (
            "self.read_back_native_amx_plan_manifests_under_publication_guard(",
            "self.write_native_amx_participant_application_receipt_artifact_"
            "only_with_retention_policy_under_publication_guard(",
        ),
        (
            "self.write_native_amx_participant_application_receipt_artifact_"
            "only_with_retention_policy_under_publication_guard(",
            "self.write_native_amx_participant_receipt_latest_index_"
            "for_prepublication_under_publication_guard(",
        ),
        (
            "self.write_native_amx_participant_receipt_latest_index_"
            "for_prepublication_under_publication_guard(",
            "self.authenticate_native_amx_participant_application_"
            "prepublication_under_publication_guard(",
        ),
    ),
    ids=(
        "all-manifests-before-all-receipts",
        "all-manifests-before-manifest-readback",
        "manifest-readback-before-all-receipts",
        "all-receipts-before-all-latest-indexes",
        "all-latest-indexes-before-readback-auth",
    ),
)
def test_native_prepublication_contract_rejects_kura_phase_order_drift(
    tmp_path: Path, earlier: str, later: str
) -> None:
    module = load_checker()
    models = copy_native_prepublication_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/kura.rs"
    swap_ordered_once(path, earlier, later)
    errors = validate_native_prepublication_fixture(tmp_path, module, models)
    assert any(
        "ordered Native prepublication item "
        "persist_native_amx_participant_application_evidence_"
        "under_publication_guard" in error
        for error in errors
    ), errors


def test_native_prepublication_contract_rejects_prewsv_retention_cleanup(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_native_prepublication_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/kura.rs"
    replace_once(
        path,
        "const fn permits_retention_cleanup(self) -> bool {\n"
        "        matches!(self, Self::PostWsvRepair)\n"
        "    }",
        "const fn permits_retention_cleanup(self) -> bool {\n"
        "        matches!(self, Self::PreWsv)\n"
        "    }",
    )
    errors = validate_native_prepublication_fixture(tmp_path, module, models)
    assert any(
        "permits_retention_cleanup must authorize only PostWsvRepair" in error
        for error in errors
    ), errors


def test_native_prepublication_contract_rejects_unguarded_retention_cleanup(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_native_prepublication_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/kura.rs"
    replace_once(path, "if permit_cleanup {", "if true {")
    errors = validate_native_prepublication_fixture(tmp_path, module, models)
    assert any(
        "cleanup-only-after-WSV" in error
        or (
            "ordered Native prepublication item "
            "persist_native_amx_participant_application_evidence_"
            "under_publication_guard" in error
        )
        for error in errors
    ), errors


def test_native_prepublication_contract_rejects_writer_retention_guard_weakening(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_native_prepublication_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/kura.rs"
    replace_once(
        path,
        "if !permit_retention_cleanup {\n"
        "            self.require_native_amx_evidence_prune_intent_absent_locked"
        "(&namespace)?;\n"
        "        }",
        "if false {\n"
        "            self.require_native_amx_evidence_prune_intent_absent_locked"
        "(&namespace)?;\n"
        "        }",
    )
    errors = validate_native_prepublication_fixture(tmp_path, module, models)
    assert any(
        "must fail closed on retention state before PostWsvRepair" in error
        for error in errors
    ), errors


def test_native_prepublication_contract_rejects_repair_mode_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_native_prepublication_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/kura.rs"
    source = path.read_text(encoding="utf-8")
    repair_offset = source.index(
        "pub(crate) fn repair_native_amx_participant_application_evidence"
    )
    mode_offset = source.index(
        "NativeAmxParticipantApplicationPublicationMode::PostWsvRepair",
        repair_offset,
    )
    path.write_text(
        source[:mode_offset]
        + "NativeAmxParticipantApplicationPublicationMode::PreWsv"
        + source[
            mode_offset
            + len(
                "NativeAmxParticipantApplicationPublicationMode::PostWsvRepair"
            ) :
        ],
        encoding="utf-8",
    )
    errors = validate_native_prepublication_fixture(tmp_path, module, models)
    assert any(
        "post-WSV Native repair must not use PreWsv publication mode" in error
        for error in errors
    ), errors


def test_inflight_layout_contract_accepts_current_production(tmp_path: Path) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    assert validate_fixture(tmp_path, module, contract) == ()


@pytest.mark.parametrize(
    ("relative", "old", "new"),
    (
        (
            Path(
                "formal/sumeragi_v2/"
                "SumeragiV2InFlightFirstRelease.tla"
            ),
            "LaneExecutablePayloadV1",
            "LaneExecutablePayloadV3",
        ),
        (
            Path(
                "formal/sumeragi_v2/"
                "SumeragiV2InFlightFirstRelease.tla"
            ),
            "SelectQueuePlanV4Conjunction ==\n",
            "SelectQueuePlanV4Snapshot ==\n",
        ),
        (
            Path("crates/iroha_core/src/lane_consensus.rs"),
            "LANE_EXECUTABLE_PAYLOAD_VERSION_V2: u8 = 2",
            "LANE_EXECUTABLE_PAYLOAD_VERSION_V2: u8 = 3",
        ),
        (
            Path("crates/iroha_core/src/queue/journal.rs"),
            "QUEUE_PLAN_JOURNAL_VERSION: u16 = 4",
            "QUEUE_PLAN_JOURNAL_VERSION: u16 = 5",
        ),
        (
            Path("crates/iroha_core/src/queue/reservation_journal.rs"),
            "LANE_QUEUE_RESERVATION_JOURNAL_VERSION: u16 = 5",
            "LANE_QUEUE_RESERVATION_JOURNAL_VERSION: u16 = 9",
        ),
        (
            Path("crates/iroha_data_model/src/merge.rs"),
            "MAX_MERGE_EXECUTION_ENTRYPOINTS: usize = 4_096",
            "MAX_MERGE_EXECUTION_ENTRYPOINTS: usize = 4_097",
        ),
        (
            Path("crates/iroha_core/src/kura.rs"),
            "pub entrypoint_hashes: Vec<Hash>,\n"
            "    /// Accepted entrypoints in lane descriptor order.\n"
            "    pub entrypoints: Vec<TransactionEntrypoint>,\n"
            "    /// Exact durable queue reservation identities in entrypoint order.\n"
            "    pub reservation_keys: Vec<LaneQueueReservationKeyV2>",
            "pub entrypoint_hashes: Vec<Hash>,\n"
            "    /// Accepted entrypoints in lane descriptor order.\n"
            "    pub entrypoints: Vec<TransactionEntrypoint>,\n"
            "    /// Exact durable queue reservation identities in entrypoint order.\n"
            "    pub reservation_tokens: Vec<LaneQueueReservationKeyV2>",
        ),
        (
            Path("scripts/formal/run_sumeragi_v2_inflight_first_release.sh"),
            "MLPayloadSchemaV2CarriesExactAdmissionPreimage",
            "MLPayloadV3CarriesExactAdmissionPreimage",
        ),
        (
            Path("scripts/formal/run_sumeragi_v2_inflight_first_release.sh"),
            'local invariant_marker="Error: Invariant ${invariant} is violated."',
            'local invariant_marker="Invariant ${invariant} is violated."',
        ),
        (
            Path("scripts/formal/run_sumeragi_v2_inflight_first_release.sh"),
            'sumeragi_v2_tlc_assert_exact_line \\\n'
            '    "$config" "$log" "$invariant_marker"',
            'grep -Fq "$invariant_marker" "$log"',
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '"inflight_first_release_fixed.cfg",\n        "18",',
            '"inflight_first_release_fixed.cfg",\n        "17",',
        ),
        (
            Path("formal/sumeragi_v2/inflight_first_release_fixed.cfg"),
            "INVARIANT MLQueuePlanV4SelectedConjunctionBound4096\n",
            "",
        ),
    ),
)
def test_inflight_layout_contract_rejects_semantic_drift(
    tmp_path: Path, relative: Path, old: str, new: str
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    replace_once(tmp_path / relative, old, new)
    assert validate_fixture(tmp_path, module, contract)


def copy_mutation_runner_fixture(tmp_path: Path, module) -> Path:
    relative = module.TLC_MUTATION_RUNNER_RELATIVE
    destination = tmp_path / relative
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(ROOT_DIR / relative, destination)
    return destination


def validate_mutation_runner_fixture(tmp_path: Path, module) -> tuple[str, ...]:
    errors: list[str] = []
    module._validate_mutation_runner(tmp_path, canonical_models(), errors)
    return tuple(errors)


def test_multilane_mutation_runner_accepts_shared_exact_line_contract(
    tmp_path: Path,
) -> None:
    module = load_checker()
    copy_mutation_runner_fixture(tmp_path, module)
    assert validate_mutation_runner_fixture(tmp_path, module) == ()


@pytest.mark.parametrize(
    ("old", "new"),
    (
        (
            'local invariant_marker="Error: Invariant ${invariant} is violated."',
            'local invariant_marker="Invariant ${invariant} is violated."',
        ),
        (
            'sumeragi_v2_tlc_assert_exact_line "$name" "$log" "$invariant_marker"',
            'grep -Fq "$invariant_marker" "$log"',
        ),
        (
            'sumeragi_v2_tlc_assert_terminal "$name" "$log"',
            'grep -Fq "Finished in" "$log"',
        ),
    ),
)
def test_multilane_mutation_runner_rejects_weakened_result_contract(
    tmp_path: Path, old: str, new: str
) -> None:
    module = load_checker()
    runner = copy_mutation_runner_fixture(tmp_path, module)
    replace_once(runner, old, new)
    assert validate_mutation_runner_fixture(tmp_path, module)


def test_inflight_layout_contract_rejects_durability_order_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue.rs"
    source = path.read_text(encoding="utf-8")
    assert source.count("durable_claim.global_admission_binding()") >= 1
    assert source.count("journal.put_batch(") >= 1
    source = source.replace("journal.put_batch(", "journal.put_all(", 1)
    source = source.replace(
        "durable_claim.global_admission_binding()",
        "journal.put_batch(Vec::new()); "
        "durable_claim.global_admission_binding()",
        1,
    )
    path.write_text(source, encoding="utf-8")
    errors = validate_fixture(tmp_path, module, contract)
    assert any("missing or reorders token" in error for error in errors)


def test_inflight_layout_contract_rejects_reservation_pre_state_identity_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue/reservation_journal.rs"
    replace_once(
        path,
        "expected_state_identity: self.checked_state_identity,",
        "expected_state_identity: resulting_state_identity,",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "IndexedReservationReplayState::prepare_checked_transition"
        in error
        and "expected_state_identity: self.checked_state_identity" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("old", "new", "expected_token"),
    (
        (
            "authorization_domain: self.authorization_domain.authorization(),",
            "authorization_domain: Arc::new(()),",
            "authorization_domain: self.authorization_domain.authorization()",
        ),
        (
            "expected_shape: self.checked_shape(),",
            "expected_shape: CheckedReplayStateShape { "
            "live: 0, committed: 0, release_barriers: 0, "
            "completed_releases: 0, ownership: 0, fifo_ordinals: 0, "
            "live_lane_incarnations: 0, next_order: 0 },",
            "expected_shape: self.checked_shape()",
        ),
        (
            ".authorizes(&prepared.authorization_domain)",
            ".authorizes(&self.authorization_domain.authorization())",
            ".authorizes(&prepared.authorization_domain)",
        ),
        (
            "self.checked_shape() != prepared.expected_shape",
            "self.checked_shape() == prepared.expected_shape",
            "self.checked_shape() != prepared.expected_shape",
        ),
    ),
)
def test_inflight_layout_contract_rejects_state_instance_and_shape_binding_drift(
    tmp_path: Path,
    old: str,
    new: str,
    expected_token: str,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue/reservation_journal.rs"
    replace_once(path, old, new)
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        expected_token in error
        and (
            "PreparedReservationJournalTransition" in error
            or "prepare_checked_transition" in error
            or "apply_checked_transition" in error
        )
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("old", "new", "symbol", "expected_token"),
    (
        (
            "fn clone(&self) -> Self {\n        Self::default()\n    }",
            "fn clone(&self) -> Self {\n"
            "        Self(Arc::clone(&self.0))\n"
            "    }",
            "CheckedReplayAuthorizationDomain::clone",
            "Self::default()",
        ),
        (
            "Arc::ptr_eq(&self.0, authorization)",
            "true",
            "CheckedReplayAuthorizationDomain::authorizes",
            "Arc::ptr_eq(&self.0, authorization)",
        ),
    ),
)
def test_inflight_layout_contract_rejects_authorization_domain_weakening(
    tmp_path: Path,
    old: str,
    new: str,
    symbol: str,
    expected_token: str,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue/reservation_journal.rs"
    replace_once(path, old, new)
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        symbol in error and expected_token in error for error in errors
    ), errors


@pytest.mark.parametrize(
    ("field", "projection"),
    (
        ("live", "self.live.len()"),
        ("committed", "self.committed.len()"),
        ("release_barriers", "self.release_barriers.len()"),
        ("completed_releases", "self.completed_releases.len()"),
        ("ownership", "self.ownership.len()"),
        ("fifo_ordinals", "self.fifo_ordinals.len()"),
        (
            "live_lane_incarnations",
            "self.live_by_lane_incarnation.len()",
        ),
        ("next_order", "self.next_order"),
    ),
)
def test_inflight_layout_contract_rejects_checked_shape_projection_weakening(
    tmp_path: Path,
    field: str,
    projection: str,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue/reservation_journal.rs"
    replace_once(path, f"{field}: {projection},", f"{field}: Default::default(),")
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "IndexedReservationReplayState::checked_shape" in error
        and f"{field}: {projection}" in error
        for error in errors
    ), errors


def test_inflight_layout_contract_rejects_checked_shape_field_extension(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue/reservation_journal.rs"
    replace_once(
        path,
        "struct CheckedReplayStateShape {\n    live: usize,",
        "struct CheckedReplayStateShape {\n"
        "    unrelated_cache_entries: usize,\n"
        "    live: usize,",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "CheckedReplayStateShape" in error
        and "missing current-layout token" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("old", "new", "expected_token"),
    (
        (
            "IN_FLIGHT_RESERVATION_ACTION_RECOVER_SNAPSHOT,\n"
            "                        key,\n"
            "                        release_digest,\n"
            "                        self.ownership.get(&hash).copied(),\n"
            "                        candidate.ownership.get(&hash).copied(),",
            "IN_FLIGHT_RESERVATION_ACTION_RECOVER_SNAPSHOT,\n"
            "                        key,\n"
            "                        release_digest,\n"
            "                        self.ownership.get(&hash).copied(),\n"
            "                        self.ownership.get(&hash).copied(),",
            "candidate.ownership.get(&hash).copied()",
        ),
        (
            "let after = before.or(Some(DurableReservationOwnership::Live(key)));",
            "let after = Some(DurableReservationOwnership::Live(key));",
            "let after = before.or(Some(DurableReservationOwnership::Live(key)));",
        ),
        (
            "let after = if before == Some(DurableReservationOwnership::Live(*key)) {\n"
            "                        None\n"
            "                    } else {\n"
            "                        before\n"
            "                    };",
            "let after = before;",
            "let after = if before == Some(DurableReservationOwnership::Live(*key)) {",
        ),
        (
            "IN_FLIGHT_RESERVATION_ACTION_COMMIT,\n"
            "                    *key,\n"
            "                    None,\n"
            "                    before,\n"
            "                    Some(DurableReservationOwnership::Committed(*key)),",
            "IN_FLIGHT_RESERVATION_ACTION_COMMIT,\n"
            "                    *key,\n"
            "                    None,\n"
            "                    before,\n"
            "                    before,",
            "IN_FLIGHT_RESERVATION_ACTION_COMMIT,",
        ),
        (
            "let after = if before == "
            "Some(DurableReservationOwnership::Committed(*key)) {\n"
            "                    None\n"
            "                } else {\n"
            "                    before\n"
            "                };",
            "let after = before;",
            "let after = if before == "
            "Some(DurableReservationOwnership::Committed(*key)) {",
        ),
        (
            "IN_FLIGHT_RESERVATION_ACTION_PRUNE_RETIRED,\n"
            "                        before.key(),\n"
            "                        None,\n"
            "                        Some(before),\n"
            "                        None,",
            "IN_FLIGHT_RESERVATION_ACTION_PRUNE_RETIRED,\n"
            "                        before.key(),\n"
            "                        None,\n"
            "                        Some(before),\n"
            "                        Some(before),",
            "IN_FLIGHT_RESERVATION_ACTION_PRUNE_RETIRED,",
        ),
        (
            "Some(DurableReservationOwnership::Live(existing)) "
            "if existing == *key => {\n"
            "                            "
            "Some(DurableReservationOwnership::Prepared {\n"
            "                                key: *key,\n"
            "                                barrier_digest: release_digest,\n"
            "                            })\n"
            "                        }",
            "Some(DurableReservationOwnership::Live(_existing)) => {\n"
            "                            "
            "Some(DurableReservationOwnership::Prepared {\n"
            "                                key: *key,\n"
            "                                barrier_digest: release_digest,\n"
            "                            })\n"
            "                        }",
            "Some(DurableReservationOwnership::Live(existing)) "
            "if existing == *key => {",
        ),
        (
            "}) if existing == key && barrier_digest == release_digest => {",
            "}) if existing == key => {",
            "barrier_digest == release_digest",
        ),
        (
            "let after = if has_completion\n"
            "                        && before\n"
            "                            == "
            "Some(DurableReservationOwnership::Completed {\n"
            "                                key: *key,\n"
            "                                barrier_digest: release_digest,\n"
            "                            }) {\n"
            "                        None\n"
            "                    } else {\n"
            "                        before\n"
            "                    };",
            "let after = before;",
            "let after = if has_completion",
        ),
    ),
    ids=(
        "snapshot",
        "reserve",
        "release-direct",
        "commit",
        "forget-commit",
        "prune",
        "prepare-release",
        "complete-release",
        "forget-release",
    ),
)
def test_inflight_layout_contract_rejects_action_owner_projection_drift(
    tmp_path: Path,
    old: str,
    new: str,
    expected_token: str,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue/reservation_journal.rs"
    replace_once(path, old, new)
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "IndexedReservationReplayState::check_in_flight_transition" in error
        and expected_token in error
        for error in errors
    ), errors


def test_inflight_layout_contract_rejects_reordered_owner_token_coverage(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue/reservation_journal.rs"
    swap_ordered_once(
        path,
        "prepared.owner_transition_count != prepared.owner_transitions.len()",
        "checked_transition_coverage_identity(&prepared.owner_transitions)?",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "IndexedReservationReplayState::apply_checked_transition" in error
        and "missing or reorders token "
        "'checked_transition_coverage_identity(&prepared.owner_transitions)?'"
        in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("earlier", "later", "rejected_token"),
    (
        (
            "self.transition_semantics(frame, maximum, false)?;",
            "let current_owner_transitions = "
            "self.check_in_flight_transition(frame, maximum)?;",
            "let current_owner_transitions = "
            "self.check_in_flight_transition(frame, maximum)?;",
        ),
        (
            "for checked in prepared.owner_transitions",
            "self.transition_semantics(frame, maximum, true)?;",
            "checked.into_projection()",
        ),
        (
            "self.transition_semantics(frame, maximum, true)?;",
            "self.transition_generation = prepared.next_generation;",
            "self.transition_generation = prepared.next_generation;",
        ),
    ),
)
def test_inflight_layout_contract_rejects_revalidate_consume_apply_order_drift(
    tmp_path: Path,
    earlier: str,
    later: str,
    rejected_token: str,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue/reservation_journal.rs"
    swap_ordered_once(path, earlier, later)
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "IndexedReservationReplayState::apply_checked_transition" in error
        and f"missing or reorders token {rejected_token!r}" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("injected", "forbidden"),
    (
        ("let _candidate = self.clone();", "self.clone()"),
        ("let _candidate = Clone::clone(self);", "Clone::clone(self)"),
        ("let _candidate = (*self).clone();", "(*self).clone()"),
        ("let _candidate = self.to_owned();", "self.to_owned()"),
        (
            "let _candidate = ToOwned::to_owned(self);",
            "ToOwned::to_owned(self)",
        ),
        (
            "let _ = candidate.transition_semantics("
            "frame, maximum, true)?;",
            "candidate.transition_semantics(",
        ),
        ("*self = candidate;", "*self = candidate"),
    ),
    ids=(
        "clone-method",
        "clone-trait",
        "clone-deref",
        "to-owned-method",
        "to-owned-trait",
        "candidate-transition",
        "candidate-swap",
    ),
)
def test_inflight_layout_contract_rejects_unbounded_full_state_application(
    tmp_path: Path,
    injected: str,
    forbidden: str,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue/reservation_journal.rs"
    replace_once(
        path,
        "self.transition_semantics(frame, maximum, true)?;",
        f"{injected}\n        self.transition_semantics(frame, maximum, true)?;",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "IndexedReservationReplayState::apply_checked_transition" in error
        and "forbidden source-bound token" in error
        and forbidden in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("symbol", "earlier", "later"),
    (
        (
            "IndexedReservationReplayState::transition_release_batch",
            ".push(self.validate_live_secondary_indexes("
            "key.signed_transaction_hash, *key)?)",
            "if apply {",
        ),
        (
            "IndexedReservationReplayState::transition_commit",
            "Some(self.validate_live_secondary_indexes("
            "key.signed_transaction_hash, existing)?)",
            "if !apply {",
        ),
        (
            "IndexedReservationReplayState::transition_prune",
            "removals.push(self.validate_live_secondary_indexes(hash, key)?);",
            "if apply {",
        ),
        (
            "IndexedReservationReplayState::transition_complete_release",
            "let live_record = "
            "self.validate_live_secondary_indexes(hash, record.key)?;",
            "if apply {",
        ),
    ),
)
def test_inflight_layout_contract_rejects_removal_before_full_preflight(
    tmp_path: Path,
    symbol: str,
    earlier: str,
    later: str,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue/reservation_journal.rs"
    swap_ordered_once(path, earlier, later)
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        symbol in error and "missing or reorders token" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("old", "new", "expected_token"),
    (
        (
            "expected_key.signed_transaction_hash != hash",
            "false",
            "expected_key.signed_transaction_hash != hash",
        ),
        (
            "let record = self\n"
            "            .live\n"
            "            .get(&hash)\n"
            "            .ok_or_else(|| "
            'invalid_data("live reservation index has no exact record"))?;',
            "let record = self\n"
            "            .live\n"
            "            .values()\n"
            "            .next()\n"
            "            .ok_or_else(|| "
            'invalid_data("live reservation index has no exact record"))?;',
            ".get(&hash)",
        ),
        (
            "self.fifo_ordinals.get(&record.value.fifo_order.ordinal) "
            "!= Some(&hash)",
            "self.fifo_ordinals\n"
            "            .get(&record.value.fifo_order.ordinal)\n"
            "            .is_some_and(|existing| existing != &hash)",
            "self.fifo_ordinals.get(&record.value.fifo_order.ordinal) "
            "!= Some(&hash)",
        ),
        (
            ".is_some_and(|hashes| hashes.contains(&hash))",
            ".is_some_and(|_hashes| true)",
            ".is_some_and(|hashes| hashes.contains(&hash))",
        ),
    ),
)
def test_inflight_layout_contract_rejects_secondary_index_preflight_weakening(
    tmp_path: Path,
    old: str,
    new: str,
    expected_token: str,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue/reservation_journal.rs"
    replace_once(path, old, new)
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "IndexedReservationReplayState::validate_live_secondary_indexes"
        in error
        and expected_token in error
        for error in errors
    ), errors


def test_inflight_layout_contract_rejects_panicking_preflighted_removal(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue/reservation_journal.rs"
    replace_once(
        path,
        "self.live.remove(&hash);",
        'self.live.remove(&hash).expect("unchecked live removal");',
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "IndexedReservationReplayState::remove_preflighted_live" in error
        and "forbidden source-bound token 'expect('" in error
        for error in errors
    ), errors


def test_inflight_layout_contract_rejects_legacy_unchecked_removal(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue/reservation_journal.rs"
    replace_once(
        path,
        "self.remove_preflighted_live(record);",
        "self.remove_live_unchecked(record.key.signed_transaction_hash);",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "IndexedReservationReplayState::transition_release_batch" in error
        and "remove_live_unchecked" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("earlier", "later", "rejected_token"),
    (
        (
            ".prepare_checked_transition("
            "frame, self.limits.max_owned_transactions)?",
            "self.append_staged(&encoded, expected_end, prepared)",
            "encode_frame_with_limit(frame, "
            "self.limits.max_frame_payload_bytes)?",
        ),
        (
            "self.append_staged(&encoded, expected_end, prepared)",
            "if let Err(error) = "
            "self.replay_state.apply_checked_transition(",
            "if let Err(error) = "
            "self.replay_state.apply_checked_transition(",
        ),
        (
            "// replay instead of panicking or attempting an in-process retry.",
            "self.poisoned = true;",
            "self.poisoned = true;",
        ),
    ),
)
def test_inflight_layout_contract_rejects_append_publication_order_drift(
    tmp_path: Path,
    earlier: str,
    later: str,
    rejected_token: str,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue/reservation_journal.rs"
    swap_ordered_once(path, earlier, later)
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "LaneQueueReservationJournal::append_durable" in error
        and f"missing or reorders token {rejected_token!r}" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("earlier", "later", "rejected_token"),
    (
        (
            "self.parent.sync_all()",
            "compacted_replay_state.apply_checked_transition(\n"
            "                    frame,\n"
            "                    self.limits.max_owned_transactions,\n"
            "                    prepared,\n"
            "                )",
            "compacted_replay_state.apply_checked_transition(",
        ),
        (
            "// The replacement is already durable. Keep the previous",
            "self.replay_state = compacted_replay_state;",
            "self.poisoned = true;",
        ),
    ),
)
def test_inflight_layout_contract_rejects_compaction_publication_order_drift(
    tmp_path: Path,
    earlier: str,
    later: str,
    rejected_token: str,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue/reservation_journal.rs"
    swap_ordered_once(path, earlier, later)
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        "LaneQueueReservationJournal::compact_if_needed" in error
        and f"missing or reorders token {rejected_token!r}" in error
        for error in errors
    ), errors


def test_inflight_layout_contract_rejects_capability_restart_test_name_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = (
        tmp_path
        / "crates/iroha_core/src/queue/reservation_journal_recovery_tests.rs"
    )
    test_name = (
        "fn runtime_commit_requires_live_owner_but_snapshot_recovery_may_"
        "restore_commit_barrier()"
    )
    replace_once(
        path,
        test_name,
        test_name.replace("restore_commit_barrier", "restore_any_commit_barrier"),
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        f"current-layout token {test_name!r} must occur exactly once, found 0"
        in error
        for error in errors
    ), errors


def test_inflight_layout_contract_rejects_ledger_weakening(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    contract["source_checks"][0]["required_tokens"].pop()
    copy_layout_fixture(tmp_path, module, canonical_contract())
    errors = validate_fixture(tmp_path, module, contract)
    assert any("whole-file source checks differ" in error for error in errors)


def test_inflight_layout_contract_rejects_action_inventory_weakening(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    contract["required_actions"].remove("PersistPlanTombstone")
    copy_layout_fixture(tmp_path, module, canonical_contract())
    errors = validate_fixture(tmp_path, module, contract)
    assert any("actions differ" in error for error in errors)


def test_inflight_layout_contract_rejects_refinement_claim_inflation(
    tmp_path: Path,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    module_path = (
        tmp_path
        / "formal"
        / "sumeragi_v2"
        / "SumeragiV2InFlightFirstRelease.tla"
    )
    replace_once(
        module_path,
        "InFlightFirstReleaseSpec == Init /\\ [][Next]_vars",
        "InFlightFirstReleaseProductionRefinementObligation == TRUE\n\n"
        "InFlightFirstReleaseSpec == Init /\\ [][Next]_vars",
    )
    errors = validate_fixture(tmp_path, module, contract)
    assert any("must not declare a production refinement" in error for error in errors)
