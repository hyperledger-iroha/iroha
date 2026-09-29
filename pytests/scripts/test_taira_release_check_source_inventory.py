"""Early selected source inventory checks; no Cargo or live network required."""

import importlib.util
from pathlib import Path
import re
import shutil
import tempfile
import unittest
from unittest.mock import patch


SCRIPT = Path(__file__).resolve().parents[2] / "scripts/taira_release_check.py"
SPEC = importlib.util.spec_from_file_location("taira_release_check_source_inventory", SCRIPT)
gate = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(gate)


class SelectedSourceInventoryTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        helper = Path("scripts/formal/rust_text.py")
        (self.root / helper).parent.mkdir(parents=True)
        shutil.copyfile(SCRIPT.parent / "formal/rust_text.py", self.root / helper)

    def source(self, package, path, text):
        source = self.root / "crates" / package / path
        source.parent.mkdir(parents=True, exist_ok=True)
        source.write_text(text)

    def test_direct_and_generated_test_declarations_cover_selected_stages(self):
        self.source("iroha_torii", "src/routing.rs", """
            routing_test! { async selected_route { } }
            #[tokio::test] async fn selected_torii_unit() { }
        """)
        self.source("iroha_torii", "tests/taira_app_contracts.rs", """
            #[tokio::test] async fn selected_torii_integration() { }
        """)
        self.source("iroha_core", "src/lib.rs", """
            state_test! { sync selected_state { } }
            state_test!(consensus_stack selected_stack, {});
            source_contract_test!(selected_contract);
            world_test!(selected_apply, {});
            scenario_test!(selected_scenario, "F35", scenarios::f35);
        """)
        stages = {
            "torii-unit": (("router", ("routing::tests::selected_route",
                                        "routing::tests::selected_torii_unit")),),
            "torii": (("endpoint", ("tests::selected_torii_integration",)),),
            "core": (("reducer", ("sumeragi::selected_state", "sumeragi::selected_stack",
                                    "sumeragi::selected_contract", "sumeragi::selected_apply",
                                    "sim::tests::selected_scenario")),),
        }
        gate.validate_selected_source_test_inventory(self.root, stages)

    def test_explicit_cargo_test_target_is_required_when_autotests_are_disabled(self):
        self.source("iroha_config", "tests/fixtures.rs", "#[test] fn actual_case() {}")
        self.source("iroha_config", "tests/iroha_config_integration.rs", "mod fixtures;")
        manifest = self.root / "crates/iroha_config/Cargo.toml"
        manifest.write_text('[package]\nname="iroha_config"\nautotests=false\n'
                            '[[test]]\nname="iroha_config_integration"\npath="tests/iroha_config_integration.rs"\n')
        stages = {"config-fixtures": (("current fixture", ("fixtures::actual_case",)),)}
        gate.validate_selected_source_test_inventory(self.root, stages)
        stale = ("stale fixture target", "fixtures", "test", ["-p", "iroha_config", "--test", "fixtures"])
        with patch.dict(gate.HARNESS_TARGETS, {"config-fixtures": stale}):
            with self.assertRaisesRegex(gate.CheckError, r"autotests=false.*target fixtures"):
                gate.validate_selected_source_test_inventory(self.root, stages)
        manifest.write_text(manifest.read_text().replace('tests/iroha_config_integration.rs', 'tests/missing.rs'))
        with self.assertRaisesRegex(gate.CheckError, "target source is absent"):
            gate.validate_selected_source_test_inventory(self.root, stages)

    def test_missing_torii_integration_function_fails_before_cargo(self):
        self.source("iroha_torii", "src/lib.rs", "#[test] fn unrelated() {}")
        stages = {"torii": (("endpoint", ("tests::exact_missing_torii_case",)),)}
        with self.assertRaisesRegex(gate.CheckError,
                                    "torii: tests::exact_missing_torii_case"):
            gate.validate_selected_source_test_inventory(self.root, stages)

    def test_missing_selection_stops_lifecycle_checks_before_subprocesses(self):
        self.source("iroha_torii", "src/lib.rs", "#[test] fn unrelated() {}")
        stages = {"torii": (("endpoint", ("tests::exact_missing_torii_case",)),)}
        with patch.object(gate, "validate_mv_test_registration") as mv, \
             patch.object(gate.subprocess, "run") as subprocess, \
             patch.object(gate, "validate_native_consensus_test_registration") as standalone:
            with self.assertRaisesRegex(gate.CheckError,
                                        "torii: tests::exact_missing_torii_case"):
                gate.run_lifecycle_source_checks(self.root, {}, (), stages)
        mv.assert_not_called()
        subprocess.assert_not_called()
        standalone.assert_not_called()

    def test_comments_and_literals_are_not_declarations(self):
        self.source("iroha_torii", "src/lib.rs", '''
            // #[test] fn absent_from_comment() {}
            const DESCRIPTION: &str = r#"routing_test! { sync absent_from_literal {} }"#;
        ''')
        stages = {"torii-unit": (("router", ("tests::absent_from_comment",
                                                 "tests::absent_from_literal")),)}
        with self.assertRaisesRegex(gate.CheckError, "2 selected test declarations absent"):
            gate.validate_selected_source_test_inventory(self.root, stages)

    def test_same_leaf_in_another_package_does_not_satisfy_selection(self):
        self.source("iroha_core", "src/lib.rs", "#[test] fn other_package_case() {}")
        self.source("iroha_torii", "src/lib.rs", "#[test] fn unrelated() {}")
        stages = {"torii-unit": (("router", ("routing::other_package_case",)),)}
        with self.assertRaisesRegex(gate.CheckError, "torii-unit: routing::other_package_case"):
            gate.validate_selected_source_test_inventory(self.root, stages)

    def test_duplicate_selected_name_fails(self):
        stages = {"torii": (("first", ("tests::duplicate",)),
                             ("second", ("tests::duplicate",)))}
        with self.assertRaisesRegex(gate.CheckError, "repeats a test in torii"):
            gate.validate_selected_source_test_inventory(self.root, stages)

    def test_missing_inventory_reports_every_case_in_one_preflight(self):
        self.source("iroha_torii", "src/lib.rs", "#[test] fn unrelated() {}")
        missing = tuple(f"tests::absent_{index}" for index in range(28))
        with self.assertRaises(gate.CheckError) as failure:
            gate.validate_selected_source_test_inventory(
                self.root, {"torii-unit": (("current handlers", missing),)})
        diagnostic = str(failure.exception)
        self.assertIn("28 selected test declarations absent", diagnostic)
        for name in missing:
            self.assertIn("torii-unit: " + name, diagnostic)

    def test_current_checkout_census_has_real_source_declarations_in_both_scopes(self):
        # Exercise the actual checkout, not a second list of expected strings.
        # Refactors must update the release census before source capture or Cargo.
        for scope in gate.QUALIFICATION_SCOPES:
            with self.subTest(scope=scope):
                gate.validate_selected_source_test_inventory(
                    SCRIPT.parents[1], gate.qualification_stages(scope))

    def test_public_rate_materialization_is_required_in_the_real_cli_harness(self):
        root = SCRIPT.parents[1]
        source = root / "crates/iroha_cli/src/taira_public_reset.rs"
        self.assertRegex(source.read_text(),
                         r'#\[path = "taira_public_reset_validator_config\.rs"\]\s*mod validator_config;')
        name = ("taira_public_reset::validator_config::tests::"
                "materialization_replaces_localnet_admission_with_distinct_public_clients")
        self.assertEqual(gate.HARNESS_TARGETS["cli"][3],
                         ["-p", "iroha_cli", "--bin", "iroha"])
        for scope in gate.QUALIFICATION_SCOPES:
            with self.subTest(scope=scope):
                selected = gate.qualification_stages(scope)["cli"]
                names = tuple(case for _, cases in selected for case in cases)
                self.assertEqual(names.count(name), 1)
                focused = gate.focused_regression_stages(scope, ("cli=" + name,))
                self.assertEqual(tuple(focused), ("cli",))
                self.assertEqual(tuple(case for _, cases in focused["cli"] for case in cases), (name,))
                gate.validate_selected_source_test_inventory(root, focused)
                listing = "\n".join(case + ": test" for case in names if case != name)
                with self.assertRaisesRegex(gate.CheckError, "required regressions missing"):
                    gate.require_tests(listing, selected)

    def test_prepared_signature_fixture_equality_is_required_in_the_torii_harness(self):
        root = SCRIPT.parents[1]
        source = (root / "crates/iroha_torii/src/routing.rs").read_text()
        self.assertIn("mod prepared_transaction_signature_fixture_tests {", source)
        body = re.search(r"routing_test!\s*\{\s*sync prepared_transaction_signature_fixture_is_current\b(.*?)\n    \}",
                         source, re.DOTALL).group(1)
        self.assertIn("let fixture = build_fixture();", body)
        self.assertIn("/../../fixtures/prepared_transactions/prepared_transaction_signature_v1.json", body)
        self.assertIn("assert_eq!(fixture, expected);", body)
        name = ("routing::prepared_transaction_signature_fixture_tests::"
                "prepared_transaction_signature_fixture_is_current")
        self.assertEqual(gate.HARNESS_TARGETS["torii-unit"][3],
                         ["-p", "iroha_torii", "--lib"])
        for scope in gate.QUALIFICATION_SCOPES:
            with self.subTest(scope=scope):
                selected = gate.qualification_stages(scope)["torii-unit"]
                names = tuple(case for _, cases in selected for case in cases)
                self.assertEqual(names.count(name), 1)
                focused = gate.focused_regression_stages(scope, ("torii-unit=" + name,))
                self.assertEqual(tuple(focused), ("torii-unit",))
                self.assertEqual(tuple(case for _, cases in focused["torii-unit"] for case in cases), (name,))
                gate.validate_selected_source_test_inventory(root, focused)
                listing = "\n".join(case + ": test" for case in names if case != name)
                with self.assertRaisesRegex(gate.CheckError, "required regressions missing"):
                    gate.require_tests(listing, selected)

    def test_owner_authority_and_schema_suites_bind_real_bodies_to_native_harnesses(self):
        root = SCRIPT.parents[1]
        self.assertIn('include!("executor_contract_owner_permission_tests.rs");',
                      (root / "crates/iroha_core/src/executor.rs").read_text())
        core_source = root / "crates/iroha_core/src/executor_contract_owner_permission_tests.rs"
        core_names = tuple("executor::tests::" + name for name in re.findall(
            r"#\[test\]\s*fn\s+([A-Za-z_]\w*)\s*\(", core_source.read_text()))
        self.assertEqual(len(core_names), 3)
        groups = {
            "core": core_names,
            "schema": tuple(name for _, names in gate.SCHEMA_STAGES for name in names),
        }
        for scope in gate.QUALIFICATION_SCOPES:
            selected = gate.qualification_stages(scope)
            _, plan, _ = gate.native_harness_plan(selected, ())
            for harness, required in groups.items():
                with self.subTest(scope=scope, harness=harness):
                    self.assertIn(harness, plan)
                    names = tuple(name for _, cases in selected[harness] for name in cases)
                    gate.validate_selected_source_test_inventory(root, {harness: selected[harness]})
                    for name in required:
                        self.assertEqual(names.count(name), 1)
                        focused = gate.focused_regression_stages(scope, (harness + "=" + name,))
                        self.assertEqual(tuple(focused), (harness,))
                        listing = "\n".join(case + ": test" for case in names if case != name)
                        with self.assertRaisesRegex(gate.CheckError, "required regressions missing"):
                            gate.require_tests(listing, selected[harness])
        self.assertEqual(gate.HARNESS_TARGETS["schema"][3], ["-p", "iroha_schema_gen", "--lib"])

    def test_current_per_seat_bootstrap_suite_is_selected_once_with_real_test_bodies(self):
        root = SCRIPT.parents[1]
        module = root / "crates/irohad/src/beacon_bootstrap.rs"
        source = root / "crates/irohad/src/beacon_bootstrap_tests.rs"
        self.assertRegex(module.read_text(),
                         r'#\[path = "beacon_bootstrap_tests\.rs"\]\s*mod tests;')
        declared = tuple(re.findall(r"#\[test\]\s*fn\s+([A-Za-z_]\w*)\s*\(",
                                    source.read_text()))
        self.assertEqual(len(declared), 14)
        for scope in gate.QUALIFICATION_SCOPES:
            selected = tuple(name for _, tests in gate.qualification_stages(scope)["daemon"]
                             for name in tests)
            for leaf in declared:
                name = "beacon_bootstrap::tests::" + leaf
                with self.subTest(scope=scope, test=name):
                    self.assertEqual(selected.count(name), 1)
                    without_case = "\n".join(case + ": test" for case in selected if case != name)
                    with self.assertRaisesRegex(gate.CheckError, "required regressions missing"):
                        gate.require_tests(without_case, gate.qualification_stages(scope)["daemon"])

    def test_genesis_identity_publication_controls_are_required_in_both_scopes(self):
        cli_source = SCRIPT.parents[1] / "crates/iroha_cli/src"
        self.assertRegex((cli_source / "main_shared.rs").read_text(), r"(?m)^mod taira;")
        self.assertRegex((cli_source / "taira.rs").read_text(),
                         r'#\[path = "taira_parliament_seating\.rs"\]\s*pub\(crate\) mod parliament_seating;')
        self.assertRegex((cli_source / "taira_parliament_seating.rs").read_text(),
                         r'#\[path = "taira_parliament_seating_tests\.rs"\]\s*mod tests;')
        required = {"kagami": (
            "genesis::sign::tests::identity_drift_leaves_every_requested_output_unchanged",
            "genesis::sign::tests::expected_hash_output_matches_the_signed_consensus_header",
            "genesis::sign::tests::network_identity_publication_is_idempotent_and_refuses_drift",
            "genesis::sign::tests::existing_network_identity_requires_safe_single_link_custody",
            "genesis::sign::tests::guarded_replacement_publishes_consistent_genesis_bundle",
            "genesis::sign::tests::guarded_replacement_rejects_stale_missing_and_unsafe_prior_without_writes",
            "genesis::sign::tests::identity_guard_serializes_publishers_and_rejects_substitution",
            "genesis::sign::tests::interrupted_replacement_preserves_prior_identity_until_complete_retry",
            "genesis::sign::tests::replacement_requires_complete_explicit_output_bundle",
        ), "cli": (
            "taira::parliament_seating::tests::seat_parliament_seats_a_generated_network_once",
            "taira::parliament_seating::tests::resign_identity_requires_one_canonical_line",
        )}
        for scope in gate.QUALIFICATION_SCOPES:
            for harness, names in required.items():
                stages = gate.qualification_stages(scope)[harness]
                selected = tuple(name for _, tests in stages for name in tests)
                gate.validate_selected_source_test_inventory(SCRIPT.parents[1], {harness: stages})
                for name in names:
                    with self.subTest(scope=scope, harness=harness, test=name):
                        self.assertEqual(selected.count(name), 1)
                        focused = gate.focused_regression_stages(scope, (harness + "=" + name,))
                        self.assertEqual(tuple(focused), (harness,))
                        missing = "\n".join(case + ": test" for case in selected if case != name)
                        with self.assertRaisesRegex(gate.CheckError, "required regressions missing"):
                            gate.require_tests(missing, stages)

    def test_cli_seating_selectors_follow_actual_module_aliases_and_entrypoint(self):
        sources = {
            "Cargo.toml": '[package]\nname = "iroha_cli"\n[[bin]]\nname = "iroha"\npath = "src/bin/iroha.rs"\n',
            "src/bin/iroha.rs": 'include!("../main_shared.rs");\n',
            "src/main_shared.rs": 'mod taira;\n',
            "src/taira.rs": '#[path = "taira_parliament_seating.rs"]\npub(crate) mod parliament_seating;\n',
            "src/taira_parliament_seating.rs": '#[cfg(test)]\n#[path = "taira_parliament_seating_tests.rs"]\nmod tests;\n',
            "src/taira_parliament_seating_tests.rs": '#[test]\nfn seating_case() {}\n',
        }
        def write_sources():
            for path, text in sources.items():
                self.source("iroha_cli", path, text)
        def selected(name):
            return {"cli": (("seating", (name,)),)}
        correct = "taira::parliament_seating::tests::seating_case"
        write_sources()
        gate.validate_selected_source_test_inventory(self.root, selected(correct))
        for wrong in ("taira_parliament_seating::tests::seating_case", "unrelated::seating_case"):
            with self.subTest(selector=wrong), self.assertRaisesRegex(
                    gate.CheckError, "CLI seating selector lacks its registered module route"):
                gate.validate_selected_source_test_inventory(self.root, selected(wrong))

        # The guard derives the alias from source; it cannot merely compare
        # against a second hard-coded copy of the expected selector prefix.
        self.source("iroha_cli", "src/taira.rs", sources["src/taira.rs"].replace(
            "mod parliament_seating", "mod seated_parliament"))
        with self.assertRaisesRegex(gate.CheckError, "registered module route"):
            gate.validate_selected_source_test_inventory(self.root, selected(correct))
        gate.validate_selected_source_test_inventory(
            self.root, selected("taira::seated_parliament::tests::seating_case"))

        for path, replacement in (
            ("Cargo.toml", sources["Cargo.toml"].replace("src/bin/iroha.rs", "src/bin/unused.rs")),
            ("Cargo.toml", sources["Cargo.toml"] + '[[bin]]\nname = "iroha"\npath = "src/bin/iroha.rs"\n'),
            ("src/bin/iroha.rs", 'include!("../other.rs");\n'),
            ("src/main_shared.rs", '#[path = "foreign.rs"]\nmod taira;\n'),
            ("src/taira.rs", '/*\n' + sources["src/taira.rs"] + '*/\n'),
            ("src/taira_parliament_seating.rs", 'const DECOY: &str = r#"\n'
             + sources["src/taira_parliament_seating.rs"] + '"#;\n'),
        ):
            with self.subTest(edge=path):
                write_sources()
                self.source("iroha_cli", path, replacement)
                with self.assertRaisesRegex(gate.CheckError, "CLI seating"):
                    gate.validate_selected_source_test_inventory(self.root, selected(correct))
        write_sources()
        for target in (
            ("foreign package", "iroha", "bin", ["-p", "other", "--bin", "iroha"]),
            ("foreign binary", "other", "bin", ["-p", "iroha_cli", "--bin", "other"]),
        ):
            with self.subTest(target=target), patch.dict(gate.HARNESS_TARGETS, {"cli": target}):
                with self.assertRaisesRegex(gate.CheckError, "CLI seating Cargo target registration"):
                    gate.validate_selected_source_test_inventory(self.root, selected(correct))


if __name__ == "__main__":
    unittest.main()
