"""Exact native source ownership rejects stale declarations and wrong module edges."""
import importlib.util
from pathlib import Path
import shutil
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("taira_native_inventory_test", ROOT / "scripts/taira_native_test_inventory.py")
inventory = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(inventory)


class NativeInventoryTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        helper = self.root / "scripts/formal/rust_text.py"
        helper.parent.mkdir(parents=True)
        shutil.copyfile(ROOT / "scripts/formal/rust_text.py", helper)
        self.package = self.root / "crates/iroha_core/src"
        self.package.mkdir(parents=True)
        self.parent = self.package / "owner.rs"
        self.source = self.package / "tests.rs"
        self.parent.write_text('#[path = "tests.rs"]\nmod tests;\n')
        self.source.write_text('#[test]\nfn exact_case() {}\n')
        self.owners = (("authority", "owner.rs", "tests.rs", "tests", "owner::tests", ("exact_case",)),)

    def validate(self):
        return inventory.validate_native_source_inventory(self.root, owners=self.owners)

    def test_exact_owner_and_registered_source(self):
        self.assertEqual(self.validate(), {"owner::tests::exact_case"})

    def test_missing_extra_duplicate_and_comment_cases_fail(self):
        for source in ('', '#[test]\nfn other() {}', '#[test]\nfn exact_case() {}\n#[test]\nfn extra() {}',
                       '#[test]\nfn exact_case() {}\n#[test]\nfn exact_case() {}',
                       '// #[test]\n// fn exact_case() {}', 'const X: &str = "#[test] fn exact_case() {}";'):
            with self.subTest(source=source):
                self.source.write_text(source)
                with self.assertRaisesRegex(ValueError, "census differs"):
                    self.validate()

    def test_wrong_module_path_and_comment_registration_fail(self):
        for source in ('#[path = "other.rs"]\nmod tests;', '// #[path = "tests.rs"]\n// mod tests;',
                       'mod other;', '#[path = "tests.rs"] mod tests; #[path = "tests.rs"] mod tests;',
                       '// #[path = "tests.rs"]\nmod tests;',
                       '/* #[path = "tests.rs"] */ #[path = "foreign.rs"] mod tests;',
                       '#[path = "tests.rs"] #[path = "foreign.rs"] mod tests;'):
            with self.subTest(source=source):
                self.parent.write_text(source)
                with self.assertRaisesRegex(ValueError, "registration differs"):
                    self.validate()

    def test_inline_include_is_bound_to_its_registered_module(self):
        self.parent.write_text('mod tests { include!("tests.rs"); }')
        self.assertEqual(self.validate(), {"owner::tests::exact_case"})
        self.parent.write_text('mod tests {} mod other { include!("tests.rs"); }')
        with self.assertRaisesRegex(ValueError, "registration differs"):
            self.validate()

    def test_real_checkout_matches_every_reviewed_owner(self):
        names = inventory.validate_native_source_inventory(ROOT)
        self.assertEqual(len(names), sum(len(row[-1]) for row in inventory.NATIVE_CORE_TEST_OWNERS))
        self.assertEqual(len(names), 315)

    def test_current_native_owner_additions_reject_same_count_selector_replacement(self):
        expected = {
            'native beacon reporting control': (
                ('sumeragi/epoch_beacon/producer.rs', 'sumeragi/epoch_beacon/producer/readiness.rs', 'readiness', 'sumeragi::epoch_beacon::producer::readiness::tests'),
                (
                    'readiness_refresh_excludes_reader_until_validated_publication',
                    'readiness_failed_refresh_withdraws_old_positive_before_reader_returns',
                    'readiness_observation_requires_exact_even_generation_height_and_applied_cut',
                    'readiness_wait_expires_while_original_probe_guard_remains_held',
                    'readiness_poisoning_refuses_both_diagnostic_paths',
                )),
            'native publication custody': (
                ('sumeragi/executor.rs', 'sumeragi/executor_publication_tests.rs', 'publication_tests', 'sumeragi::executor::publication_tests'),
                (
                    'replay_completion_retirement_keeps_exact_source_and_original_pool_retry',
                )),
            'native completed replay identity': (
                ('sumeragi/executor/replay.rs', 'sumeragi/executor/replay/tests.rs', 'tests', 'sumeragi::executor::replay::tests'),
                (
                    'replay_completion_state_read_refusal_retains_original_receipt_and_retries_after_release',
                    'replay_committee_hash_streams_exact_counted_key_preimage',
                )),
            'native certified prefix authority': (
                ('sumeragi/certified_chain/tests.rs', 'sumeragi/certified_chain/prefix_tests.rs', 'prefix_tests', 'sumeragi::certified_chain::tests::prefix_tests'),
                (
                    'staged_certificate_prefix_preserves_reset_target_first_order_and_original_receipts',
                    'staged_certificate_prefix_refusal_preserves_cursor_and_target_before_gap_errors',
                    'scoped_reverse_walk_reads_all_original_frames_and_rechecks_corrupt_ancestors',
                    'checked_prefix_finish_matches_original_complete_step_and_authority',
                    'checked_prefix_finish_preserves_refusal_rejection_and_same_source_retry',
                    'admitted_prefix_finish_matches_original_step_and_retains_original_slot_until_finish',
                    'admitted_prefix_finish_preserves_original_pool_refusal_and_certificate_error_order',
                )),
        }
        for label, (edge, added) in expected.items():
            with self.subTest(owner=label):
                owner, = (row for row in inventory.NATIVE_CORE_TEST_OWNERS
                          if row[0] == label)
                self.assertEqual(owner[1:5], edge)
                for leaf in added:
                    self.assertEqual(owner[-1].count(leaf), 1)
                for relative in owner[1:3]:
                    target = self.package / relative
                    target.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copyfile(ROOT / "crates/iroha_core/src" / relative, target)
                source = self.package / owner[2]
                original = source.read_text()
                self.assertEqual(inventory.validate_native_source_inventory(
                    self.root, owners=(owner,)),
                    {owner[4] + "::" + leaf for leaf in owner[-1]})
                for leaf in added:
                    with self.subTest(leaf=leaf):
                        declaration = "fn " + leaf + "("
                        self.assertEqual(original.count(declaration), 1)
                        source.write_text(original.replace(
                            declaration, "fn unreviewed_native_replacement("))
                        with self.assertRaisesRegex(ValueError, "census differs") as refusal:
                            inventory.validate_native_source_inventory(self.root, owners=(owner,))
                        self.assertIn(leaf, str(refusal.exception))
                        self.assertIn("unreviewed_native_replacement", str(refusal.exception))
                source.write_text(original)

    def test_source_retries_and_replay_bind_only_current_executable_owners(self):
        expected = {
            "native execution producer source retries": (
                "sumeragi/driver/exec.rs", "sumeragi/driver/producer_retry_tests.rs",
                "producer_retry_tests", "sumeragi::driver::exec::producer_retry_tests", 12),
            "native witness admission": (
                "sumeragi/driver/mod.rs", "sumeragi/driver/tests/witness_admission.rs",
                "witness_admission_tests", "sumeragi::driver::witness_admission_tests", 11),
            "native completed replay identity": (
                "sumeragi/executor/replay.rs", "sumeragi/executor/replay/tests.rs",
                "tests", "sumeragi::executor::replay::tests", 8),
            "native dynamic VM projection refusal": (
                "pipeline/access/dynamic_execution.rs", "pipeline/access/dynamic_execution/tests.rs",
                "tests", "pipeline::access::dynamic_execution::tests", 5),
        }
        for label, (*edge, count) in expected.items():
            with self.subTest(owner=label):
                rows = [row for row in inventory.NATIVE_CORE_TEST_OWNERS if row[0] == label]
                self.assertEqual(len(rows), 1)
                self.assertEqual(rows[0][1:5], tuple(edge))
                self.assertEqual(len(rows[0][-1]), count)
        self.assertFalse(any("strict_replay_retirement_tests" in field
                             for row in inventory.NATIVE_CORE_TEST_OWNERS for field in row[:5]))

    def test_startup_and_world_verification_retain_the_current_module_edges(self):
        expected = {
            "native beacon reporting control": (
                "sumeragi/epoch_beacon/producer.rs", "sumeragi/epoch_beacon/producer/readiness.rs",
                "readiness", "sumeragi::epoch_beacon::producer::readiness::tests", 7),
            "native beacon startup failure classification": (
                "sumeragi/executor.rs", "sumeragi/executor_control.rs",
                "control", "sumeragi::executor::control::tests", 2),
            "native complete World root verification": (
                "sumeragi/test_chain.rs", "sumeragi/test_chain/world_state_tests.rs",
                "world_state_tests", "sumeragi::test_chain::tests::world_state_tests", 6),
        }
        for label, (*edge, count) in expected.items():
            with self.subTest(owner=label):
                rows = [row for row in inventory.NATIVE_CORE_TEST_OWNERS if row[0] == label]
                self.assertEqual(len(rows), 1)
                self.assertEqual(rows[0][1:5], tuple(edge))
                self.assertEqual(len(rows[0][-1]), count)

    def test_physical_history_checks_retain_the_exact_original_storage_owner(self):
        owners = [row for row in inventory.NATIVE_CORE_TEST_OWNERS
                  if row[0] == "native original transaction history custody"]
        self.assertEqual(len(owners), 1)
        owner = owners[0]
        self.assertEqual(owner[1:5], (
            "state/storage_transactions/history.rs",
            "state/storage_transactions/history_tests.rs",
            "tests",
            "state::storage_transactions::history::tests",
        ))
        self.assertEqual(len(owner[-1]), 22)
        self.assertEqual(owner[-1][:4], (
            "initial_native_tree_and_identity_have_one_exact_finite_admission",
            "independent_history_release_controls_stay_funded_through_last_observation",
            "physical_history_busy_ignores_logical_cleanup_and_retries_actual_release",
            "physical_admission_notifies_after_logical_unlock_on_success_refusal_and_unwind",
        ))

    def test_warmed_prefix_checks_have_the_exact_registered_native_owner(self):
        owners = [row for row in inventory.NATIVE_CORE_TEST_OWNERS
                  if row[0] == "native certified prefix authority"]
        self.assertEqual(len(owners), 1)
        owner = owners[0]
        self.assertEqual(owner[1:5], (
            "sumeragi/certified_chain/tests.rs",
            "sumeragi/certified_chain/prefix_tests.rs",
            "prefix_tests",
            "sumeragi::certified_chain::tests::prefix_tests",
        ))
        self.assertEqual(owner[-1], (
            "streamed_prefix_emits_genesis_execution_anchor_only_after_real_successor",
            "unsigned_changed_genesis_result_cannot_be_exported_by_streamed_reader",
            "streamed_prefix_checks_genuine_pasta_at_retained_empty_epoch_boundary",
            "warmed_epoch_shape_rejects_substituted_context_and_still_checks_each_qc",
            "warmed_reader_rechecks_durable_prefix_and_fresh_view_after_body_removal",
            "standalone_and_scoped_frame_reads_agree_without_skipping_shape_checks",
            'staged_certificate_prefix_preserves_reset_target_first_order_and_original_receipts',
            'staged_certificate_prefix_refusal_preserves_cursor_and_target_before_gap_errors',
            'scoped_reverse_walk_reads_all_original_frames_and_rechecks_corrupt_ancestors',
            'checked_prefix_finish_matches_original_complete_step_and_authority',
            'checked_prefix_finish_preserves_refusal_rejection_and_same_source_retry',
            'admitted_prefix_finish_matches_original_step_and_retains_original_slot_until_finish',
            'admitted_prefix_finish_preserves_original_pool_refusal_and_certificate_error_order',
        ))
