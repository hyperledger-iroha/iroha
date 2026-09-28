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
            v2_apply_test!(selected_apply, {});
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
             patch.object(gate, "_run_standalone_checks") as standalone:
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
        self.assertRegex((root / "crates/iroha_executor/src/default/mod.rs").read_text(),
                         r"#\[cfg\(test\)\]\s*mod contract_deployment_permission_tests;")
        core_source = root / "crates/iroha_core/src/executor_contract_owner_permission_tests.rs"
        core_names = tuple("executor::tests::" + name for name in re.findall(
            r"#\[test\]\s*fn\s+([A-Za-z_]\w*)\s*\(", core_source.read_text()))
        self.assertEqual(len(core_names), 3)
        groups = {
            "core": core_names,
            "executor": tuple(name for _, names in gate.EXECUTOR_STAGES for name in names),
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
        for harness, package in (("executor", "iroha_executor"), ("schema", "iroha_schema_gen")):
            self.assertEqual(gate.HARNESS_TARGETS[harness][3], ["-p", package, "--lib"])

    def test_current_per_seat_bootstrap_suite_is_selected_once_with_real_test_bodies(self):
        root = SCRIPT.parents[1]
        module = root / "crates/irohad/src/beacon_bootstrap.rs"
        source = root / "crates/irohad/src/beacon_bootstrap_tests.rs"
        self.assertRegex(module.read_text(),
                         r'#\[path = "beacon_bootstrap_tests\.rs"\]\s*mod tests;')
        declared = tuple(re.findall(r"#\[test\]\s*fn\s+([A-Za-z_]\w*)\s*\(",
                                    source.read_text()))
        self.assertEqual(len(declared), 12)
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


if __name__ == "__main__":
    unittest.main()
