"""Real formatter and fail-closed controls for formatting-only coverage decisions."""

import importlib.util
from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("formatting_tests_guard", ROOT / "ci/check_tests_guard.py")
GUARD = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = GUARD
SPEC.loader.exec_module(GUARD)


class FormattingCoverageTests(unittest.TestCase):
    def same(self, before, after):
        return GUARD.formatting_only_change(before, after, root=ROOT)

    def test_real_formatter_accepts_wrapping_and_trailing_comma(self):
        self.assertTrue(self.same(
            'fn send() { call(one, two, three); }\n',
            'fn send() {\n    call(\n        one,\n        two,\n        three,\n    );\n}\n',
        ))

    def test_unresolved_child_is_not_read_or_written(self):
        self.assertTrue(self.same('mod absent; fn f(){}', 'mod absent;\nfn f() {}\n'))
        self.assertFalse((ROOT / 'absent.rs').exists())

    def test_changed_behavior_requires_coverage(self):
        for before, after in [
            ('fn f() -> u32 { 1 }', 'fn f() -> u32 { 2 }'),
            ('fn f() { allow(); }', 'fn f() { deny(); }'),
            ('#[cfg(unix)] fn f() {}', '#[cfg(windows)] fn f() {}'),
            ('fn f(){ let s = "a b"; }', 'fn f(){ let s = "ab"; }'),
            ('fn f(){ let s = r#"a b"#; }', 'fn f(){ let s = r#"ab"#; }'),
        ]:
            with self.subTest(before=before, after=after):
                self.assertFalse(self.same(before, after))

    def test_new_removed_and_unparseable_sources_require_coverage(self):
        for before, after in [('', 'fn f() {}'), ('fn f() {}', ''),
                              ('fn f( {', 'fn f() {}'), ('fn f() {}', 'fn f( {')]:
            with self.subTest(before=before, after=after):
                self.assertFalse(self.same(before, after))

    def test_missing_formatter_and_timeout_require_coverage(self):
        for error in [FileNotFoundError(), subprocess.TimeoutExpired('rustfmt', 60)]:
            with self.subTest(error=type(error).__name__):
                with patch.object(GUARD.subprocess, 'run', side_effect=error):
                    self.assertFalse(self.same('fn f(){}', 'fn f() {}'))

    def test_empty_or_failed_output_never_exempts_coverage(self):
        for code, output in [(0, ''), (1, 'fn f() {}\n')]:
            result = subprocess.CompletedProcess(['rustfmt'], code, output, '')
            with patch.object(GUARD.subprocess, 'run', return_value=result):
                self.assertFalse(self.same('fn f(){}', 'fn f() {}'))


if __name__ == '__main__':
    unittest.main()
