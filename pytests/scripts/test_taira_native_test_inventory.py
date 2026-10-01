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
        self.assertEqual(len(names), 165)
