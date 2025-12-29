#!/usr/bin/env python3
"""
Regression tests for ci/check_tests_guard.py.
"""

from __future__ import annotations

import importlib.machinery
import importlib.util
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = REPO_ROOT / "ci" / "check_tests_guard.py"


def load_guard():
    spec = importlib.util.spec_from_loader(
        "check_tests_guard",
        importlib.machinery.SourceFileLoader("check_tests_guard", str(MODULE_PATH)),
    )
    if spec is None or spec.loader is None:
        raise RuntimeError("Unable to load check_tests_guard module.")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def run(cmd: list[str], *, cwd: Path) -> None:
    result = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)
    if result.returncode != 0:
        raise RuntimeError(
            f"Command failed ({cmd})\nstdout:\n{result.stdout}\nstderr:\n{result.stderr}"
        )


def write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


class FunctionsMappingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.guard = load_guard()

    def test_maps_changed_lines_to_function(self) -> None:
        diff_lines = [
            "@@ -1,3 +1,3 @@ pub fn add() {",
            "-    let x = 1;",
            "+    let x = 2;",
            "}",
        ]
        added, removed, inline = self.guard.parse_changed_lines(diff_lines)
        self.assertFalse(inline)
        head_ranges = self.guard.find_function_ranges(
            textwrap.dedent(
                """\
                pub fn add() {
                    let x = 2;
                }
                """
            )
        )
        functions = self.guard.functions_for_lines(added, head_ranges)
        self.assertIn("add", functions)

    def test_extract_test_only_text_strips_prod_definitions(self) -> None:
        content = textwrap.dedent(
            """\
            pub fn add_one(value: u32) -> u32 {
                value + 1
            }

            #[cfg(test)]
            mod tests {
                use super::*;

                #[test]
                fn untouched_identity() {
                    assert_eq!(5, untouched(5));
                }
            }
            """
        )
        extracted = self.guard.extract_test_only_text(content)
        self.assertIn("untouched_identity", extracted)
        self.assertNotIn("pub fn add_one", extracted)


class TestsGuardTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.guard = load_guard()

    def setUp(self) -> None:
        self.temp_dir = tempfile.TemporaryDirectory()
        self.repo = Path(self.temp_dir.name)
        run(["git", "init"], cwd=self.repo)
        run(["git", "config", "user.email", "tests@example.com"], cwd=self.repo)
        run(["git", "config", "user.name", "Tests Guard"], cwd=self.repo)
        run(["git", "config", "commit.gpgsign", "false"], cwd=self.repo)

    def tearDown(self) -> None:
        self.temp_dir.cleanup()

    def init_crate(self) -> tuple[Path, str]:
        write(
            self.repo / "Cargo.toml",
            textwrap.dedent(
                """\
                [workspace]
                members = ["foo"]
                """
            ),
        )
        crate_root = self.repo / "foo"
        crate_root.mkdir()
        write(
            crate_root / "Cargo.toml",
            textwrap.dedent(
                """\
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2024"
                """
            ),
        )
        (crate_root / "src").mkdir()
        write(
            crate_root / "src/lib.rs",
            textwrap.dedent(
                """\
                pub fn add_one(value: u32) -> u32 {
                    value + 1
                }

                pub fn untouched(value: u32) -> u32 {
                    value
                }
                """
            ),
        )
        run(["git", "add", "."], cwd=self.repo)
        run(["git", "commit", "-m", "initial"], cwd=self.repo)
        base_commit = (
            subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=self.repo, text=True)
            .strip()
        )
        return crate_root, base_commit

    def test_detects_changed_function_without_tests(self) -> None:
        crate_root, base_commit = self.init_crate()

        write(
            crate_root / "src/lib.rs",
            textwrap.dedent(
                """\
                pub fn add_one(value: u32) -> u32 {
                    value + 2
                }

                pub fn untouched(value: u32) -> u32 {
                    value
                }
                """
            ),
        )
        run(["git", "add", "."], cwd=self.repo)
        run(["git", "commit", "-m", "bump impl"], cwd=self.repo)

        violations = self.guard.gather_violations(base_commit, root=self.repo)
        self.assertEqual(1, len(violations))
        violation = violations[0]
        self.assertEqual(Path("foo/src/lib.rs"), violation.path)
        self.assertIn("add_one", violation.functions)

        exit_code = self.guard.main(["--base", base_commit, "--repo-root", str(self.repo)])
        self.assertEqual(1, exit_code)

    def test_passes_when_tests_updated(self) -> None:
        crate_root, base_commit = self.init_crate()

        write(
            crate_root / "src/lib.rs",
            textwrap.dedent(
                """\
                pub fn add_one(value: u32) -> u32 {
                    value + 2
                }
                """
            ),
        )
        run(["git", "add", "."], cwd=self.repo)
        run(["git", "commit", "-m", "modify function"], cwd=self.repo)

        tests_dir = crate_root / "tests"
        tests_dir.mkdir()
        write(
            tests_dir / "add_one.rs",
            textwrap.dedent(
                """\
                #[test]
                fn adds_one() {
                    assert_eq!(3, foo::add_one(1));
                }
                """
            ),
        )
        run(["git", "add", "."], cwd=self.repo)
        run(["git", "commit", "-m", "add coverage"], cwd=self.repo)

        violations = self.guard.gather_violations(base_commit, root=self.repo)
        self.assertFalse(violations)

        exit_code = self.guard.main(["--base", base_commit, "--repo-root", str(self.repo)])
        self.assertEqual(0, exit_code)

    def test_passes_with_existing_coverage_reference(self) -> None:
        crate_root, base_commit = self.init_crate()

        tests_dir = crate_root / "tests"
        tests_dir.mkdir()
        write(
            tests_dir / "add_one.rs",
            textwrap.dedent(
                """\
                #[test]
                fn adds_one() {
                    assert_eq!(3, foo::add_one(1));
                }
                """
            ),
        )
        run(["git", "add", "."], cwd=self.repo)
        run(["git", "commit", "-m", "seed coverage"], cwd=self.repo)
        covered_base = (
            subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=self.repo, text=True)
            .strip()
        )

        write(
            crate_root / "src/lib.rs",
            textwrap.dedent(
                """\
                pub fn add_one(value: u32) -> u32 {
                    if value == 0 { 0 } else { value + 1 }
                }
\
                pub fn untouched(value: u32) -> u32 {
                    value
                }
                """
            ),
        )
        run(["git", "add", "."], cwd=self.repo)
        run(["git", "commit", "-m", "change impl without touching tests"], cwd=self.repo)

        violations = self.guard.gather_violations(covered_base, root=self.repo)
        self.assertFalse(violations)

        exit_code = self.guard.main(["--base", covered_base, "--repo-root", str(self.repo)])
        self.assertEqual(0, exit_code)

    def test_fails_when_tests_do_not_reference_changed_function(self) -> None:
        crate_root, base_commit = self.init_crate()

        tests_dir = crate_root / "tests"
        tests_dir.mkdir()
        write(
            tests_dir / "untouched.rs",
            textwrap.dedent(
                """\
                #[test]
                fn untouched_is_identity() {
                    assert_eq!(5, foo::untouched(5));
                }
                """
            ),
        )
        run(["git", "add", "."], cwd=self.repo)
        run(["git", "commit", "-m", "unrelated coverage"], cwd=self.repo)
        covered_base = (
            subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=self.repo, text=True)
            .strip()
        )

        write(
            crate_root / "src/lib.rs",
            textwrap.dedent(
                """\
                pub fn add_one(value: u32) -> u32 {
                    value + 3
                }

                pub fn untouched(value: u32) -> u32 {
                    value
                }
                """
            ),
        )
        run(["git", "add", "."], cwd=self.repo)
        run(["git", "commit", "-m", "change add_one without touching coverage"], cwd=self.repo)

        violations = self.guard.gather_violations(covered_base, root=self.repo)
        self.assertTrue(violations)
        violation_paths = {v.path for v in violations}
        self.assertIn(Path("foo/src/lib.rs"), violation_paths)

        exit_code = self.guard.main(["--base", covered_base, "--repo-root", str(self.repo)])
        self.assertEqual(1, exit_code)

    def test_inline_tests_do_not_bypass_missing_coverage(self) -> None:
        crate_root, base_commit = self.init_crate()

        write(
            crate_root / "src/lib.rs",
            textwrap.dedent(
                """\
                pub fn add_one(value: u32) -> u32 {
                    value + 3
                }

                pub fn untouched(value: u32) -> u32 {
                    value
                }

                #[cfg(test)]
                mod tests {
                    use super::*;

                    #[test]
                    fn untouched_identity() {
                        assert_eq!(10, untouched(10));
                    }
                }
                """
            ),
        )
        run(["git", "add", "."], cwd=self.repo)
        run(["git", "commit", "-m", "change fn and add unrelated inline test"], cwd=self.repo)

        violations = self.guard.gather_violations(base_commit, root=self.repo)
        self.assertTrue(violations)
        self.assertIn(
            Path("foo/src/lib.rs"),
            {violation.path for violation in violations},
        )

        exit_code = self.guard.main(["--base", base_commit, "--repo-root", str(self.repo)])
        self.assertEqual(1, exit_code)


if __name__ == "__main__":
    unittest.main()
