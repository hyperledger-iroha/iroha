"""Static negative controls for the autonomous recovery/capacity contract."""

from __future__ import annotations

import copy
import importlib.util
import json
import shutil
import sys
import tempfile
import unittest
from pathlib import Path


ROOT_DIR = Path(__file__).resolve().parents[2]
CHECKER = (
    ROOT_DIR
    / "scripts"
    / "formal"
    / "check_sumeragi_v2_autonomous_recovery_capacity_contract.py"
)
CONTRACT = (
    ROOT_DIR
    / "formal"
    / "sumeragi_v2"
    / "autonomous_recovery_capacity_source_bindings.json"
)
MODEL = (
    ROOT_DIR
    / "formal"
    / "sumeragi_v2"
    / "SumeragiV2AutonomousRecoveryCapacity.tla"
)


def load_checker():
    spec = importlib.util.spec_from_file_location(
        "sumeragi_v2_autonomous_recovery_capacity_contract", CHECKER
    )
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class AutonomousRecoveryCapacityContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.checker = load_checker()
        cls.contract = json.loads(CONTRACT.read_text(encoding="utf-8"))
        cls.model = MODEL.read_text(encoding="utf-8")

    def test_current_static_contract_passes(self) -> None:
        self.assertEqual(self.checker.validate_repository(ROOT_DIR), [])

    def assert_source_mutation_rejected(
        self,
        relative: str,
        old: str,
        new: str,
        expected_binding: str,
        occurrence: int = 1,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for source_relative in self.checker._manifest_relatives(self.contract):
                source = ROOT_DIR / source_relative
                target = root / source_relative
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(source, target)
            target = root / relative
            source = target.read_text(encoding="utf-8")
            parts = source.split(old)
            self.assertGreater(len(parts), occurrence)
            mutated = old.join(parts[:occurrence]) + new + old.join(parts[occurrence:])
            target.write_text(mutated, encoding="utf-8")
            errors = self.checker.validate_repository(root)
            self.assertTrue(
                any(expected_binding in error for error in errors),
                errors,
            )

    def test_completed_contract_cannot_reintroduce_editor_placeholder(self) -> None:
        contract = copy.deepcopy(self.contract)
        contract["editor_placeholders"].append(
            {
                "id": "certified_frontier_pair_bundle_capacity_obligation",
                "invariant": "MLCertifiedFrontierCapacityReconstructable",
                "expected_owner_path": "crates/iroha_core/src/kura.rs",
                "status": "editor_in_progress",
                "required_semantics": [
                    "persist pair and bundle capacity obligations with the certified frontier",
                    "reconstruct both capacity envelopes after crash before reopening startup",
                ],
            }
        )
        errors: list[str] = []
        self.checker._validate_contract_data(ROOT_DIR, contract, errors)
        self.assertTrue(
            any("ids differ from the exact pending inventory" in error for error in errors),
            errors,
        )

    def test_completed_contract_cannot_regress_integration_status(self) -> None:
        contract = copy.deepcopy(self.contract)
        contract["integration_status"] = "static_model_complete_production_bindings_pending"
        errors: list[str] = []
        self.checker._validate_contract_data(ROOT_DIR, contract, errors)
        self.assertTrue(
            any("integration_status" in error for error in errors),
            errors,
        )

    def test_route_latest_only_mutation_edge_is_required(self) -> None:
        mutated = self.model.replace(
            'THEN "None"\n       ELSE carrierNSource',
            'THEN carrierNSource\n       ELSE carrierNSource',
            1,
        )
        self.assertNotEqual(mutated, self.model)
        errors: list[str] = []
        self.checker._validate_model_source(mutated, errors)
        self.assertTrue(
            any(
                "AdvanceRouteSnapshotToNPlusOne" in error
                and 'THEN "None"' in error
                for error in errors
            ),
            errors,
        )

    def test_hash_only_predecessor_mutation_edge_is_required(self) -> None:
        mutated = self.model.replace(
            'autonomousPredecessorAdmitted\' =\n'
            '       (Mode = "HashOnlyAutonomousPredecessor")',
            "autonomousPredecessorAdmitted' = FALSE",
            1,
        )
        self.assertNotEqual(mutated, self.model)
        errors: list[str] = []
        self.checker._validate_model_source(mutated, errors)
        self.assertTrue(
            any(
                "ObserveHashOnlyAutonomousPredecessor" in error
                and "autonomousPredecessorAdmitted" in error
                for error in errors
            ),
            errors,
        )

    def test_prune_peak_after_mutation_edge_is_required(self) -> None:
        mutated = self.model.replace(
            '(pruneCapacityPeakAdmitted \\/ Mode = "PrunePeakAfterMutation")',
            "pruneCapacityPeakAdmitted",
            1,
        )
        self.assertNotEqual(mutated, self.model)
        errors: list[str] = []
        self.checker._validate_model_source(mutated, errors)
        self.assertTrue(
            any(
                "BeginPruneDurableMutation" in error
                and "PrunePeakAfterMutation" in error
                for error in errors
            ),
            errors,
        )

    def test_prune_reservation_envelope_peak_edge_is_required(self) -> None:
        mutated = self.model.replace(
            '(Mode # "PrunePeakDropsReservationEnvelope")',
            "TRUE",
            1,
        )
        self.assertNotEqual(mutated, self.model)
        errors: list[str] = []
        self.checker._validate_model_source(mutated, errors)
        self.assertTrue(
            any(
                "AdmitPruneCapacityPeak" in error
                and "PrunePeakDropsReservationEnvelope" in error
                for error in errors
            ),
            errors,
        )

    def test_autonomous_predecessor_binding_excludes_hash_only_helpers(self) -> None:
        path = (
            ROOT_DIR
            / "crates"
            / "iroha_core"
            / "src"
            / "state"
            / "autonomous_predecessor_application.rs"
        )
        source = path.read_text(encoding="utf-8")
        item, extraction_error = self.checker._extract_rust_method(
            source,
            "State::certified_autonomous_lane_block_predecessor_is_globally_applied_cached",
        )
        self.assertIsNone(extraction_error)
        self.assertIsNotNone(item)
        assert item is not None
        self.assertNotIn("hash_only_snapshot_anchor", item)
        self.assertIn("canonical_merged_lane_frontier_from_world", item)
        self.assertIn(
            "autonomous_lane_block_predecessor_merge_receipt_revalidates_without_sidecar_repair",
            item,
        )

    def test_ready_certificate_cannot_enter_ordinary_receipt_repair_binding(self) -> None:
        contract = copy.deepcopy(self.contract)
        binding = next(
            item
            for item in contract["stable_bindings"]
            if item["id"] == "autonomous_predecessor_ordinary_receipt_filter"
        )
        binding["ordered_tokens"][2] = ".is_some()"
        errors: list[str] = []
        self.checker._validate_contract_data(ROOT_DIR, contract, errors)
        self.assertTrue(
            any("ordinary_receipt_filter" in error for error in errors),
            errors,
        )

    def test_current_autonomous_receipt_revalidation_cannot_repair_sidecars(
        self,
    ) -> None:
        self.assert_source_mutation_rejected(
            "crates/iroha_core/src/kura/autonomous_application_evidence.rs",
            "        receipt.format == LaneBlockApplicationReceiptArtifactFormat::MergeExecution\n"
            "            && receipt.proposal == *proposal\n"
            "            && self\n"
            "                .lane_block_application_receipt_matches_merge_log_without_sidecar_repair(&receipt)",
            "        receipt.format == LaneBlockApplicationReceiptArtifactFormat::MergeExecution\n"
            "            && receipt.proposal == *proposal\n"
            "            && self.lane_block_application_receipt_matches_merge_log(&receipt)",
            "autonomous_current_merge_receipt_revalidator",
        )

    def test_predecessor_autonomous_receipt_revalidation_cannot_repair_sidecars(
        self,
    ) -> None:
        self.assert_source_mutation_rejected(
            "crates/iroha_core/src/kura/autonomous_application_evidence.rs",
            "            && previous.proposal_height < descriptor.proposal_height\n"
            "            && self\n"
            "                .lane_block_application_receipt_matches_merge_log_without_sidecar_repair(&receipt)",
            "            && previous.proposal_height < descriptor.proposal_height\n"
            "            && self.lane_block_application_receipt_matches_merge_log(&receipt)",
            "autonomous_predecessor_merge_receipt_revalidator",
        )

    def test_exact_incomplete_carrier_cannot_fall_back_to_route_latest(self) -> None:
        self.assert_source_mutation_rejected(
            "crates/iroha_core/src/kura/lane_artifact_budget.rs",
            ".execution_entries_for_bounded_identities(&historical_execution_identities)?",
            ".latest_execution_entry(&historical_execution_identities)?",
            "exact_incomplete_carrier_reservation_rebuild",
        )

    def test_startup_carriers_cannot_be_reauthenticated_under_sidecar_locks(
        self,
    ) -> None:
        self.assert_source_mutation_rejected(
            "crates/iroha_core/src/kura/lane_artifact_budget.rs",
            "self.ensure_post_wsv_lane_artifact_budget_reservation_after_authentication_locked(\n"
            "                pending_canonical_bytes,\n"
            "                &entry,\n"
            "                carrier.block_height,",
            "self.ensure_post_wsv_lane_artifact_budget_reservation_locked(\n"
            "                pending_canonical_bytes,\n"
            "                &entry,\n"
            "                carrier.block_height,",
            "exact_incomplete_carrier_reservation_rebuild",
        )

    def test_startup_repair_cannot_move_before_carrier_envelope_rebuild(self) -> None:
        self.assert_source_mutation_rejected(
            "crates/iroha_core/src/kura.rs",
            "                kura.rebuild_post_wsv_lane_artifact_budget_reservations_on_startup()?;\n"
            "                kura.rebuild_certified_bundle_capacity_reservations_on_startup()?;\n"
            "                kura.repair_lane_merge_application_frontiers_on_startup()?;\n"
            "                kura.rebuild_autonomous_lane_route_latest_attempt_indexes_on_startup()?;",
            "                kura.repair_lane_merge_application_frontiers_on_startup()?;\n"
            "                kura.rebuild_autonomous_lane_route_latest_attempt_indexes_on_startup()?;\n"
            "                kura.rebuild_certified_bundle_capacity_reservations_on_startup()?;\n"
            "                kura.rebuild_post_wsv_lane_artifact_budget_reservations_on_startup()?;",
            "startup_carrier_envelope_reconstruction_order",
        )

        self.assert_source_mutation_rejected(
            "crates/iroha_core/src/kura/autonomous_terminal_capacity.rs",
            "            let (used, total) = self.kura_disk_usage_bytes_with_total()?;",
            "            let _duplicate = self.kura_disk_usage_bytes_with_total();\n"
            "            let (used, total) = self.kura_disk_usage_bytes_with_total()?;",
            "debug_append_startup_accounting_publish",
        )
        self.assert_source_mutation_rejected(
            "crates/iroha_core/src/kura/autonomous_terminal_capacity.rs",
            "                self.invalidate_durable_budget_snapshot();",
            "                // Leave stale cache state visible after a rejected publication.",
            "debug_append_startup_accounting_publish",
        )
        self.assert_source_mutation_rejected(
            "crates/iroha_core/src/kura.rs",
            "        self.validate_and_publish_configured_kura_capacity_after_startup_recovery(true)?;\n"
            "        Ok(())",
            "        let _ = self\n"
            "            .validate_and_publish_configured_kura_capacity_after_startup_recovery(true);\n"
            "        Ok(())",
            "geometry_restore_combined_capacity_publish",
            2,
        )

    def test_certified_bundle_cannot_bypass_admission_before_first_write(self) -> None:
        self.assert_source_mutation_rejected(
            "crates/iroha_core/src/kura/certified_bundle_capacity.rs",
            "self.ensure_certified_bundle_capacity_reservation_under_prune_guard(",
            "self.observe_certified_bundle_capacity_reservation_after_write(",
            "certified_bundle_admission_before_first_write",
        )

    def test_certified_bundle_writer_cannot_release_route_ownership(self) -> None:
        self.assert_source_mutation_rejected(
            "crates/iroha_core/src/kura/certified_bundle_capacity.rs",
            "        // Admission and all three durable publications share one uninterrupted\n"
            "        // prune corridor.  In particular, an ordinary certificate must not be\n"
            "        // able to advance this route's frontier between installing the READY\n"
            "        // reservation and publishing its exact certified frontier/pair.\n"
            "        self.write_certified_lane_block_artifact_with_authority_under_prune_guard(",
            "        // Admission and all three durable publications share one uninterrupted\n"
            "        // prune corridor.  In particular, an ordinary certificate must not be\n"
            "        // able to advance this route's frontier between installing the READY\n"
            "        // reservation and publishing its exact certified frontier/pair.\n"
            "        self.write_certified_lane_block_artifact_with_authority(",
            "certified_bundle_admission_before_first_write",
        )

    def test_certified_bundle_plan_cannot_drop_bundle_component(self) -> None:
        self.assert_source_mutation_rejected(
            "crates/iroha_core/src/kura/certified_bundle_capacity.rs",
            "                CertifiedBundleCapacityComponent::AutonomousBundlePair,\n"
            "                bundle_component,",
            "                CertifiedBundleCapacityComponent::AutonomousBundlePair,\n"
            "                0,",
            "certified_bundle_complete_plan",
        )

    def test_certified_bundle_startup_cannot_drop_physical_crash_credit(self) -> None:
        self.assert_source_mutation_rejected(
            "crates/iroha_core/src/kura/certified_bundle_capacity.rs",
            "                    total.checked_add(reserved.saturating_sub(\n"
            "                        reservation.plan.startup_physical_credit_bytes.min(reserved),\n"
            "                    ))",
            "                    total.checked_add(reserved)",
            "certified_bundle_transactional_startup_rebuild",
        )

    def test_certified_bundle_consumption_cannot_ignore_durable_hash(self) -> None:
        self.assert_source_mutation_rejected(
            "crates/iroha_core/src/kura/certified_bundle_capacity.rs",
            "        if durable_bytes_hash != expected_hash {",
            "        if false {",
            "certified_bundle_durable_component_consumption",
        )

    def test_lane_history_compaction_cannot_count_recovered_temp_twice(self) -> None:
        self.assert_source_mutation_rejected(
            "crates/iroha_core/src/kura/lane_history_compaction.rs",
            "            let before = Self::sidecar_tracked_bytes(&data_path, &index_path)?;",
            "            let before = before_recovery;",
            "lane_history_compaction_recovery_before_capacity",
        )

    def test_prune_peak_cannot_drop_pipeline_sidecar_rewrite(self) -> None:
        self.assert_source_mutation_rejected(
            "crates/iroha_core/src/kura/prune_commit_merge_support.rs",
            "        self.marker_stable_growth_bytes\n"
            "            .checked_add(sidecar.sequential_peak_bytes)",
            "        self.marker_stable_growth_bytes.checked_add(0)",
            "prune_transaction_peak_projection",
        )

    def test_prune_peak_cannot_drop_reservation_envelope(self) -> None:
        self.assert_source_mutation_rejected(
            "crates/iroha_core/src/kura/prune_commit_merge_support.rs",
            "        self.source_physical_bytes\n"
            "            .checked_add(self.reserved_bytes()?)",
            "        self.source_physical_bytes.checked_add(0)",
            "prune_absolute_peak_projection",
        )

    def test_prune_intent_cannot_publish_before_exact_capacity_is_sealed(self) -> None:
        self.assert_source_mutation_rejected(
            "crates/iroha_core/src/kura.rs",
            "        let intent =\n"
            "            self.seal_and_validate_canonical_prune_capacity_admission(KuraPruneIntentV3 {",
            "        let intent = KuraPruneIntentV3 {",
            "prune_live_capacity_before_intent",
        )

    def test_prune_startup_cannot_repair_before_capacity_recheck(self) -> None:
        self.assert_source_mutation_rejected(
            "crates/iroha_core/src/kura.rs",
            "        if let Some(intent) = prune_intent.as_ref() {\n"
            "            kura.preflight_recovered_prune_capacity_before_mutation(intent)?;\n"
            "        }\n"
            "        if config.init_mode == InitMode::Strict {\n"
            "            kura.audit_retained_autonomous_lifecycle_cursor_generations()?;\n"
            "        }\n"
            "        if !provisional_open {\n"
            "            if config.init_mode == InitMode::Strict {\n"
            "                kura.seal_completed_autonomous_lifecycle_replica_claims_on_startup()?;\n"
            "                kura.recover_retained_block_rewrite_stage_on_startup(&blocks_root)?;",
            "        if config.init_mode == InitMode::Strict {\n"
            "            kura.audit_retained_autonomous_lifecycle_cursor_generations()?;\n"
            "        }\n"
            "        if !provisional_open {\n"
            "            if config.init_mode == InitMode::Strict {\n"
            "                kura.seal_completed_autonomous_lifecycle_replica_claims_on_startup()?;\n"
            "                kura.recover_retained_block_rewrite_stage_on_startup(&blocks_root)?;\n"
            "                if let Some(intent) = prune_intent.as_ref() {\n"
            "                    kura.preflight_recovered_prune_capacity_before_mutation(intent)?;\n"
            "                }",
            "prune_startup_capacity_before_repair",
        )

    def test_prune_recovery_split_cannot_lose_kura_include_owner(self) -> None:
        self.assert_source_mutation_rejected(
            "crates/iroha_core/src/kura.rs",
            'include!("kura/prune_recovery_capacity.rs");',
            'include!("kura/prune_recovery_capacity_unbound.rs");',
            "prune_recovery_capacity.rs",
        )

    def test_certified_bundle_split_cannot_lose_kura_include_owner(self) -> None:
        self.assert_source_mutation_rejected(
            "crates/iroha_core/src/kura.rs",
            'include!("kura/certified_bundle_capacity.rs");',
            'include!("kura/certified_bundle_capacity_unbound.rs");',
            "certified_bundle_capacity.rs",
        )

    def test_entrypoint_claim_mutation_cannot_precede_complete_peak(self) -> None:
        self.assert_source_mutation_rejected(
            "crates/iroha_core/src/kura.rs",
            "        self.preflight_autonomous_lane_entrypoint_claims_locked(\n"
            "            pending_canonical_bytes,\n"
            "            payload,\n"
            "            max_files,\n"
            "        )?;\n"
            "        let accounting_mutation = self.begin_total_disk_usage_mutation();",
            "        let accounting_mutation = self.begin_total_disk_usage_mutation();\n"
            "        self.preflight_autonomous_lane_entrypoint_claims_locked(\n"
            "            pending_canonical_bytes,\n"
            "            payload,\n"
            "            max_files,\n"
            "        )?;",
            "entrypoint_claim_set_preflight_before_mutation",
        )

    def test_normal_association_stage_bytes_cannot_leave_required_peak(self) -> None:
        self.assert_source_mutation_rejected(
            "crates/iroha_core/src/kura.rs",
            "        let mut required = budget_used\n"
            "            .saturating_add(block_required)\n"
            "            .saturating_add(merge_entry_bytes)\n"
            "            .saturating_add(association_stage_bytes);",
            "        let mut required = budget_used\n"
            "            .saturating_add(block_required)\n"
            "            .saturating_add(merge_entry_bytes)\n"
            "            .saturating_add(0);",
            "canonical_association_normal_budget_peak",
        )

    def test_replacement_association_stage_cannot_leave_joint_peak(self) -> None:
        self.assert_source_mutation_rejected(
            "crates/iroha_core/src/kura.rs",
            "        let mut required = budget_used\n"
            "            .max(projected_after)\n"
            "            .saturating_add(association_stage_bytes);",
            "        let mut required = budget_used\n"
            "            .max(projected_after)\n"
            "            .saturating_add(0);",
            "canonical_association_replacement_budget_peak",
        )

    def test_debug_append_cannot_bypass_carrier_capacity_preflight(self) -> None:
        self.assert_source_mutation_rejected(
            "crates/iroha_core/src/kura/durable_block_and_atomic_sidecar_io.rs",
            "self.validate_configured_autonomous_mutation_disk_peak_locked(",
            "self.validate_debug_bytes_after_append(",
            "debug_append_capacity_preflight_order",
        )

    def test_debug_restart_accounting_cannot_drop_bound_file_length(self) -> None:
        self.assert_source_mutation_rejected(
            "crates/iroha_core/src/kura/durable_block_and_atomic_sidecar_io.rs",
            "        Ok(metadata.len())",
            "        Ok(0)",
            "debug_append_file_accounting",
        )

    def test_split_binding_cannot_lose_kura_include_owner(self) -> None:
        self.assert_source_mutation_rejected(
            "crates/iroha_core/src/kura.rs",
            'include!("kura/merge_ledger_latest_execution_index.rs");',
            'include!("kura/merge_ledger_latest_execution_index_unbound.rs");',
            "merge_ledger_latest_execution_index.rs",
        )

    def test_source_manifest_digest_is_deterministic(self) -> None:
        first = self.checker.source_manifest_sha256(ROOT_DIR)
        second = self.checker.source_manifest_sha256(ROOT_DIR)
        self.assertEqual(first, second)
        self.assertRegex(first, r"^[0-9a-f]{64}$")


if __name__ == "__main__":
    unittest.main()
