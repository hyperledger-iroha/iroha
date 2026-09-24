"""Early selected source inventory checks; no Cargo or live network required."""

import importlib.util
from pathlib import Path
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
        helper = Path("scripts/formal/sumeragi_v2_rust_text.py")
        (self.root / helper).parent.mkdir(parents=True)
        shutil.copyfile(SCRIPT.parent / "formal/sumeragi_v2_rust_text.py", self.root / helper)

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
        """)
        stages = {
            "torii-unit": (("router", ("routing::tests::selected_route",
                                        "routing::tests::selected_torii_unit")),),
            "torii": (("endpoint", ("tests::selected_torii_integration",)),),
            "core": (("reducer", ("sumeragi::selected_state", "sumeragi::selected_stack",
                                    "sumeragi::selected_contract", "sumeragi::selected_apply")),),
        }
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


if __name__ == "__main__":
    unittest.main()
