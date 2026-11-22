import contextlib
import io
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import compile_five_kind as runner


class CompilerRunnerTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.output = self.root / "public"
        self.stages = []

    def stage(self, crate, name, env):
        self.stages.append((crate, name))
        output = Path(env[runner.TEST_OUTPUT]) if runner.TEST_OUTPUT in env else None
        if name.endswith("::compile_wallet_program_bundle"):
            output.mkdir()
            (output / "program-bundle.norito").touch()
        elif name.endswith("::issue_software_device_fixture_from_public_bundle"):
            self.assertTrue(Path(env[runner.FOUR_BUNDLE]).is_file())
            output.mkdir()
            for file in ("program-bundle.norito", "parameter-manifest.norito", "parameter-signature.bin"):
                (output / file).touch()
        elif name.startswith("host_tests::"):
            self.assertEqual(crate, runner.EXP / "device/runtime")
            self.assertEqual(Path(env[runner.INPUTS[1]]).stat().st_mode & 0o777, 0o700)
        elif name.endswith("::compile_completed_archive_programs_group_four"):
            self.assertTrue(Path(env[runner.INPUTS[2]]).is_dir())
            output.mkdir()
            for file in ("archive-primary.program", "archive-secondary.program"):
                (output / file).touch()
        elif name.endswith("::rederive_four_variable_fixed_point_public_profile"):
            previous = Path(env[runner.INPUTS[3]])
            self.assertEqual(previous.name, "final.round-1.unqualified.norito")
            self.assertTrue(previous.is_file())
            self.assertFalse(previous.samefile(Path(env["KAGEMUSHA_SPARTAN_ARCHIVE_FINAL"])))
            output.write_bytes(b"test inspection")
        else:
            self.assertEqual(name, runner.COMPILER + "fixed_point::compile_five_kind_wallet_program_bundle_group_four")
            output.mkdir()
            output.with_suffix(".round-1.unqualified.norito").write_bytes(b"test programs")
            (output / "program-bundle.norito").write_bytes(b"test programs")
            (output / "public-program-profile.json").write_bytes(b"test inspection")

    def run_mocked_compiler(self, supplied=(), bundle=None):
        with (patch.dict(os.environ, dict(supplied), clear=True),
              patch.object(runner, "test_case", self.stage),
              patch.object(runner, "export", side_effect=lambda source, out: out.mkdir()),
              patch.object(runner, "check_final") as validated,
              contextlib.redirect_stdout(io.StringIO())):
            runner.compile_five_kind(self.output, bundle)
            validated.assert_called_once()
        self.assertTrue((self.output / "program-bundle.norito").is_file())
        self.assertTrue((self.output / "public-program-profile.json").is_file())
        self.assertFalse(list(self.root.glob(".public-compiler-*")))

    def test_fresh_scope_builds_every_stage(self):
        self.run_mocked_compiler()
        self.assertEqual(len(self.stages), 6)
        self.assertEqual(self.stages[0][1], "full::backend_chain_tests::compile_wallet_program_bundle")

    def test_public_four_kind_bundle_skips_only_first_compiler(self):
        bundle = self.root / "public-four.norito"
        bundle.touch()
        self.run_mocked_compiler(bundle=bundle)
        self.assertEqual(len(self.stages), 5)
        self.assertTrue(self.stages[0][1].endswith("::issue_software_device_fixture_from_public_bundle"))

    def test_prepared_fixture_rechecks_final_compiler(self):
        supplied = {}
        for key in runner.INPUTS:
            path = self.root / key
            path.mkdir()
            supplied[key] = str(path)
        for name in ("archive-primary.program", "archive-secondary.program"):
            (Path(supplied[runner.INPUTS[3]]) / name).touch()
        self.run_mocked_compiler(supplied.items())
        self.assertEqual(len(self.stages), 2)

    def test_recheck_refuses_missing_rounds_aliased_exports_and_substituted_profile(self):
        self.output.mkdir()
        bundle = self.output / "program-bundle.norito"
        bundle.write_bytes(b"final programs")
        (self.output / "public-program-profile.json").write_bytes(b"compiler profile")
        first = self.output.with_suffix(".round-1.unqualified.norito")
        second = self.output.with_suffix(".round-2.unqualified.norito")
        second.hardlink_to(bundle)
        with self.assertRaisesRegex(ValueError, "missing consecutive"):
            runner.recheck_fixed_point(self.output, {})
        first.write_bytes(b"prior programs")
        with self.assertRaisesRegex(ValueError, "aliases"):
            runner.recheck_fixed_point(self.output, {})
        second.unlink()
        second.write_bytes(bundle.read_bytes())
        def changed_profile(crate, name, env):
            self.assertEqual(Path(env[runner.INPUTS[3]]), second)
            Path(env[runner.TEST_OUTPUT]).write_bytes(b"substituted profile")
        with (patch.object(runner, "test_case", changed_profile),
              self.assertRaisesRegex(ValueError, "independently derived")):
            runner.recheck_fixed_point(self.output, {})

    def test_existing_output_and_partial_fixture_fail_before_cargo(self):
        with patch.dict(os.environ, {runner.INPUTS[0]: str(self.root)}, clear=True):
            with self.assertRaisesRegex(ValueError, "incomplete precomputed"):
                runner.compile_five_kind(self.output, None)
        self.output.mkdir()
        with self.assertRaisesRegex(ValueError, "new directory"):
            runner.compile_five_kind(self.output, None)

    def test_cargo_success_with_zero_selected_cases_refuses(self):
        class EmptyResult:
            stdout = ["test result: ok. 0 passed; 0 failed; 0 ignored\n"]

            def wait(self):
                return 0

        with (patch.object(runner.subprocess, "Popen",
                           return_value=contextlib.nullcontext(EmptyResult())) as cargo,
              contextlib.redirect_stdout(io.StringIO()),
              self.assertRaisesRegex(ValueError, "one passing test")):
            runner.test_case(runner.EXP, "exact::case", {})
        cmd = cargo.call_args.args[0]
        self.assertIn("--exact", cmd)
        self.assertIn("--ignored", cmd)
        self.assertEqual(cmd[cmd.index("--target-dir") + 1], str(runner.EXP / "target"))


if __name__ == "__main__":
    unittest.main()
