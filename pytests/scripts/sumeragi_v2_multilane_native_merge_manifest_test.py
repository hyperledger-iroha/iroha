"""Focused negative controls for Native AMX merge-manifest projection."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

import pytest


SUPPORT_PATH = Path(__file__).with_name("sumeragi_v2_multilane_models_test.py")


def load_support():
    spec = importlib.util.spec_from_file_location(
        "sumeragi_v2_multilane_models_test_support", SUPPORT_PATH
    )
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def test_native_prepublication_contract_rejects_removed_prepublication_call(
    tmp_path: Path,
) -> None:
    support = load_support()
    module = support.load_checker()
    models = support.copy_native_prepublication_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_apply.rs"
    support.replace_once(
        path,
        ".prepublish_native_amx_participant_application_evidence(",
        ".skip_native_amx_participant_application_evidence(",
    )
    errors = support.validate_native_prepublication_fixture(
        tmp_path, module, models
    )
    assert any(
        "V2ApplyService::validate_and_apply" in error
        and "prepublish_native_amx_participant_application_evidence" in error
        for error in errors
    ), errors


def test_native_merge_manifest_contract_rejects_multiple_height_guard_removal(
    tmp_path: Path,
) -> None:
    support = load_support()
    module = support.load_checker()
    models = support.copy_native_prepublication_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/sumeragi/exec.rs"
    support.replace_once(
        path,
        "                        \"Native AMX participant route carries more than one height in one application block\"\n",
        "                        \"Native AMX participant route accepts multiple heights in one application block\"\n",
    )
    errors = support.validate_native_prepublication_fixture(
        tmp_path, module, models
    )
    assert any(
        "from_result_bearing_block_and_merge_entry" in error
        and "more than one height" in error
        for error in errors
    ), errors


def test_native_merge_manifest_contract_rejects_carrier_map_collision_guard_removal(
    tmp_path: Path,
) -> None:
    support = load_support()
    module = support.load_checker()
    models = support.copy_native_prepublication_fixture(tmp_path, module)
    path = (
        tmp_path
        / "crates/iroha_core/src/sumeragi/v2_lane_work/"
        "canonical_executed_block_application_repair.rs"
    )
    support.replace_once_after(
        path,
        "fn planned_merge_entries_by_carrier(",
        "entries.insert(key, repair.entry()).is_some()",
        "entries.insert(key, repair.entry()).is_none()",
    )
    errors = support.validate_native_prepublication_fixture(
        tmp_path, module, models
    )
    assert any(
        "planned_merge_entries_by_carrier" in error
        and "entries.insert" in error
        for error in errors
    ), errors


def test_native_merge_manifest_contract_rejects_planned_startup_witness_drop(
    tmp_path: Path,
) -> None:
    support = load_support()
    module = support.load_checker()
    models = support.copy_native_prepublication_fixture(tmp_path, module)
    path = (
        tmp_path
        / "crates/iroha_core/src/sumeragi/v2_lane_work/"
        "canonical_executed_block_application_repair.rs"
    )
    support.replace_once_after(
        path,
        "fn plan_lane_application_evidence_repair(",
        "            planned_merge_entry,\n",
        "            None,\n",
    )
    errors = support.validate_native_prepublication_fixture(
        tmp_path, module, models
    )
    assert any(
        "Native merge-manifest corridor plan_lane_application_evidence_repair"
        in error
        and "planned_merge_entry" in error
        for error in errors
    ), errors


def test_native_merge_manifest_contract_rejects_native_repair_before_merge_publication(
    tmp_path: Path,
) -> None:
    support = load_support()
    module = support.load_checker()
    models = support.copy_native_prepublication_fixture(tmp_path, module)
    path = (
        tmp_path
        / "crates/iroha_core/src/sumeragi/v2_lane_work/"
        "canonical_executed_block_application_repair.rs"
    )
    support.swap_ordered_once_after(
        path,
        "fn apply_lane_application_evidence_repair(",
        "summary.merge_carriers = kura",
        "for carrier in &plan.native_carriers",
    )
    errors = support.validate_native_prepublication_fixture(
        tmp_path, module, models
    )
    assert any(
        "Native merge-manifest corridor apply_lane_application_evidence_repair"
        in error
        for error in errors
    ), errors


def test_native_merge_manifest_contract_rejects_lost_startup_association_control(
    tmp_path: Path,
) -> None:
    support = load_support()
    module = support.load_checker()
    models = support.copy_native_prepublication_fixture(tmp_path, module)
    path = (
        tmp_path
        / "crates/iroha_core/src/sumeragi/tests/"
        "v2_apply_unsealed_01c_historical_recovery.rs"
    )
    support.replace_once_after(
        path,
        "historical_autonomous_recovery_reaches_exactly_once_canonical_merge_application",
        '"planned merge association authorizes exact Native startup repair"',
        '"startup repair no longer checks its planned merge association"',
    )
    errors = support.validate_native_prepublication_fixture(
        tmp_path, module, models
    )
    assert any(
        "Native corridor macro test" in error
        and "planned merge association" in error
        for error in errors
    ), errors

    support.shutil.copy2(
        support.ROOT_DIR
        / "crates/iroha_core/src/sumeragi/tests/"
        "v2_apply_unsealed_01c_historical_recovery.rs",
        path,
    )
    kura_path = tmp_path / "crates/iroha_core/src/kura.rs"
    transition_anchor = (
        "fn validate_native_amx_prepublication_transition_locked("
    )
    support.replace_once_after(
        kura_path,
        transition_anchor,
        "                    );\n"
        "                let durable_receipt = self",
        "                    )\n"
        "                    .ok_or_else(|| todo!())?;\n"
        "                let durable_receipt = self",
    )
    errors = support.validate_native_prepublication_fixture(
        tmp_path, module, models
    )
    assert any(
        "must permit either stable member to be absent" in error
        for error in errors
    ), errors

    support.shutil.copy2(
        support.ROOT_DIR / "crates/iroha_core/src/kura.rs", kura_path
    )
    support.replace_once_after(
        kura_path,
        transition_anchor,
        "                    || durable_receipt",
        "                    && durable_receipt",
    )
    errors = support.validate_native_prepublication_fixture(
        tmp_path, module, models
    )
    assert any(
        "must reject either present stable-member mismatch" in error
        for error in errors
    ), errors

    support.shutil.copy2(
        support.ROOT_DIR / "crates/iroha_core/src/kura.rs", kura_path
    )
    support.replace_once_after(
        kura_path,
        "fn preflight_native_amx_incoming_artifacts_locked(",
        "                && inventory.temporary(*kind).is_none()",
        "                || inventory.temporary(*kind).is_none()",
    )
    errors = support.validate_native_prepublication_fixture(
        tmp_path, module, models
    )
    assert any(
        "must reserve bytes exactly when both the stable and temporary member "
        "are absent" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("relative", "anchor", "old", "new", "symbol"),
    (
        (
            "crates/iroha_core/src/sumeragi/exec.rs",
            "fn canonical_native_amx_application_sources(",
            "|entry| merge_native_amx_application_sources(block, entry),",
            "|_entry| ordinary_native_amx_application_sources(block),",
            "canonical_native_amx_application_sources",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_apply.rs",
            "pub(crate) fn validate_candidate(",
            "            state_block.staged_merge_entry(),\n",
            "            None,\n",
            "V2ApplyService::validate_candidate",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_apply.rs",
            "fn validate_and_apply(",
            "            state_block.staged_merge_entry(),\n",
            "            None,\n",
            "V2ApplyService::validate_and_apply",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_apply.rs",
            ".prepublish_native_amx_participant_application_evidence(",
            "                        state_block.staged_merge_entry(),\n",
            "                        None,\n",
            "V2ApplyService::validate_and_apply",
        ),
        (
            "crates/iroha_core/src/state.rs",
            "pub(crate) fn native_amx_participant_frontier_markers_and_merge_entry(",
            "            merge_entry,\n        )",
            "            None,\n        )",
            "native_amx_participant_frontier_markers_and_merge_entry",
        ),
        (
            "crates/iroha_core/src/state.rs",
            "fn stage_native_amx_participant_frontiers(",
            "            self.staged_merge_entry(),\n",
            "            None,\n",
            "stage_native_amx_participant_frontiers",
        ),
        (
            "crates/iroha_core/src/state.rs",
            "fn replay_blocks_from_kura_range_inner(",
            "            state_block.staged_merge_entry(),\n",
            "            None,\n",
            "replay_blocks_from_kura_range_inner",
        ),
        (
            "crates/iroha_core/src/kura/lane_artifact_budget.rs",
            "fn lane_artifact_required_bytes_for_block(",
            "            block,\n            merge_entry,\n        )",
            "            block,\n            None,\n        )",
            "lane_artifact_required_bytes_for_block",
        ),
        (
            "crates/iroha_core/src/kura/lane_artifact_budget.rs",
            "fn native_amx_manifest_for_committed_block(",
            "            merge_entry,\n",
            "            None,\n",
            "native_amx_manifest_for_committed_block",
        ),
        (
            "crates/iroha_core/src/kura.rs",
            "fn prepublish_native_amx_participant_application_evidence(",
            "                block, false, NativeAmxMergeAssociation::Live(staged_merge_entry),\n",
            "                block, false, NativeAmxMergeAssociation::Startup(staged_merge_entry),\n",
            "prepublish_native_amx_participant_application_evidence",
        ),
        (
            "crates/iroha_core/src/kura.rs",
            "fn native_amx_participant_application_evidence_for_block_under_publication_guard(",
            "        let native_manifest = self.native_amx_manifest_for_committed_block(\n"
            "            block, merge_association, &finality,\n"
            "        )?;",
            "        let native_manifest = self.native_amx_manifest_for_committed_block(\n"
            "            block, NativeAmxMergeAssociation::CommittedOnly, &finality,\n"
            "        )?;",
            "native_amx_participant_application_evidence_for_block_under_publication_guard",
        ),
    ),
    ids=(
        "selector-drops-merge-entry",
        "proposal-commitment-drops-staged-entry",
        "apply-commitment-drops-staged-entry",
        "prepublication-drops-staged-entry",
        "frontier-projector-drops-merge-entry",
        "wsv-staging-drops-staged-entry",
        "replay-commitment-drops-staged-entry",
        "kura-budget-drops-merge-entry",
        "kura-publication-drops-committed-entry",
        "prepublication-uses-startup-association-mode",
        "publication-bypasses-selected-association-mode",
    ),
)
def test_native_prepublication_contract_rejects_merge_manifest_relation_drift(
    tmp_path: Path,
    relative: str,
    anchor: str,
    old: str,
    new: str,
    symbol: str,
) -> None:
    support = load_support()
    module = support.load_checker()
    models = support.copy_native_prepublication_fixture(tmp_path, module)
    support.replace_once_after(tmp_path / relative, anchor, old, new)
    errors = support.validate_native_prepublication_fixture(
        tmp_path, module, models
    )
    assert any(
        "Native merge-manifest relation" in error and symbol in error
        for error in errors
    ), errors
