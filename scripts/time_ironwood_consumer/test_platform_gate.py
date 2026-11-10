"""Public runner controls using synthetic xUnit/environment inputs only.

Requires Python 3.11+ standard library on macOS/Linux (validated with 3.12), with
qualify_platform.py and native_c_driver.py beside this file. No BOI_ACTIVATION_*
inputs, generated corpus or native artifacts are required. Child commands are
mocked; these tests never launch Gradle, Swift or native code, and test-data writes
are confined to temporary directories. See README.md for unittest discovery.
"""
import unittest
from unittest.mock import patch
from pathlib import Path
from types import SimpleNamespace
import tempfile
import qualify_platform as q


class PlatformGateTests(unittest.TestCase):
    def report(self, platform="swift", body="", attributes="", counters='tests="1" failures="0" errors="0" skipped="0"'):
        name, method = q.CASES[platform]
        return (f'<testsuite {counters}><testcase classname="{name}" name="{method}" {attributes}>{body}</testcase></testsuite>').encode()

    def test_accepts_exact_swift_identity(self):
        self.assertEqual(q.verify_report(self.report(), "swift")["passed"], 1)

    def test_accepts_exact_jni_identity(self):
        self.assertEqual(q.verify_report(self.report("kotlin"), "kotlin")["passed"], 1)

    def test_refuses_skipped_even_with_success_counters(self):
        with self.assertRaisesRegex(ValueError, "zero skipped"):
            q.verify_report(self.report(body="<skipped/>"), "swift")

    def test_refuses_failure_and_error(self):
        for tag in ("failure", "error"):
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                q.verify_report(self.report(body=f"<{tag}/>"), "swift")

    def test_refuses_empty_success_suite(self):
        with self.assertRaises(ValueError):
            q.verify_report(b'<testsuite tests="0" failures="0"/>', "swift")

    def test_refuses_other_test_identity(self):
        with self.assertRaisesRegex(ValueError, "identity"):
            q.verify_report(self.report().replace(b"testPinnedExternalActivation", b"testOther"), "swift")

    def test_refuses_two_tests(self):
        raw = self.report()
        case = raw[raw.index(b"<testcase"):raw.index(b"</testsuite>")]
        with self.assertRaisesRegex(ValueError, "exactly one"):
            q.verify_report(raw.replace(b"</testsuite>", case + b"</testsuite>"), "swift")

    def test_refuses_conflicting_suite_counters(self):
        for counters in ('tests="0"', 'tests="2"', 'skipped="1"', 'failures="1"'):
            with self.subTest(counters=counters), self.assertRaises(ValueError):
                q.verify_report(self.report(counters=counters), "swift")

    def test_refuses_notrun(self):
        with self.assertRaises(ValueError):
            q.verify_report(self.report(attributes='status="notrun"'), "swift")

    def test_refuses_entity_definitions(self):
        with self.assertRaises(ValueError):
            q.verify_report(b'<!DOCTYPE testsuite>' + self.report(), "swift")

    def test_absent_or_partial_inputs_never_qualify(self):
        for count in range(4):
            environment = {key: "selected" for key in q.INPUTS[:count]}
            with self.subTest(count=count), self.assertRaises(ValueError):
                q.require_selection(environment)

    def test_empty_present_and_malformed_pins_refuse(self):
        environment = dict(zip(q.INPUTS, ("/fixture", "a" * 64, "/receipt", "b" * 64)))
        q.require_selection(environment)
        for key in q.INPUTS:
            with self.subTest(key=key), self.assertRaises(ValueError):
                q.require_selection(environment | {key: ""})
        with self.assertRaises(ValueError):
            q.require_selection(environment | {q.INPUTS[1]: "not a digest"})

    def test_absence_refuses_before_any_subprocess(self):
        argv = ["qualify_platform.py", "--platform", "swift", "--source-root", "/unused",
                "--source-commit", "a" * 40, "--source-tree", "b" * 40, "--artifact-dir", "/unused",
                "--artifact-receipt", "/unused", "--artifact-receipt-sha256", "c" * 64,
                "--admission-driver-sha256", "d" * 64, "--output-dir", "/unused-new"]
        with patch("sys.argv", argv), patch.dict("os.environ", {}, clear=True), \
             patch.object(q.subprocess, "run", side_effect=AssertionError("no subprocess permitted")) as run:
            with self.assertRaisesRegex(ValueError, "requires all four"):
                q.main()
            run.assert_not_called()

    def test_rejects_all_git_overrides_including_empty_values(self):
        names = ("GIT_DIR", "GIT_WORK_TREE", "GIT_COMMON_DIR", "GIT_INDEX_FILE", "GIT_OBJECT_DIRECTORY",
                 "GIT_ALTERNATE_OBJECT_DIRECTORIES", "GIT_CONFIG", "GIT_CONFIG_COUNT", "GIT_CONFIG_KEY_0",
                 "GIT_CONFIG_PARAMETERS", "GIT_CONFIG_GLOBAL", "GIT_CONFIG_SYSTEM", "GIT_CONFIG_NOSYSTEM")
        for name in names:
            for value in ("", "private-do-not-display"):
                with self.subTest(name=name, value=bool(value)), self.assertRaises(ValueError) as caught:
                    q.validation_environment({name: value})
                self.assertNotIn("private-do-not-display", str(caught.exception))

    def test_discards_presentation_only_pager_for_all_children(self):
        self.assertEqual(q.validation_environment({"GIT_PAGER": "must-not-execute", "PATH": "/usr/bin"}),
                         {"PATH": "/usr/bin"})

    def test_source_identity_uses_explicit_environment_and_canonical_branch(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            (root / ".git").mkdir()
            replies = [str(root), "optimizations", str(root / ".git"), ".git", "H file\0", "a" * 40, "b" * 40, ""]
            environment = {"PATH": "/usr/bin", "PUBLIC_TEST": "selected"}
            with patch.object(q.subprocess, "run", side_effect=[SimpleNamespace(stdout=value) for value in replies]) as run:
                q.clean_source(root, "a" * 40, "b" * 40, environment)
                self.assertEqual(run.call_count, len(replies))
                for call in run.call_args_list:
                    self.assertEqual(call.kwargs["env"], environment)
                    self.assertEqual(call.args[0][0], "/usr/bin/git")

    def test_source_identity_refuses_alternate_top_level_and_branch(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            for replies in (["/another-checkout"], [str(root), "other-branch"]):
                with self.subTest(replies=replies), patch.object(q.subprocess, "run", side_effect=[SimpleNamespace(stdout=value) for value in replies]), self.assertRaises(ValueError):
                    q.clean_source(root, "a" * 40, "b" * 40, {})

    def test_platform_child_refuses_git_redirection_before_launch(self):
        with patch.object(q.subprocess, "run") as run, self.assertRaises(ValueError):
            q.run_command(["unused"], Path("/"), {"GIT_INDEX_FILE": ""}, Path("/unused-log"), 1)
        run.assert_not_called()

    def test_source_identity_refuses_assumed_or_skipped_index_entries(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            (root / ".git").mkdir()
            for flag in ("h", "S", "s"):
                replies = [str(root), "optimizations", str(root / ".git"), ".git", flag + " hidden\nname\0"]
                with self.subTest(flag=flag), patch.object(q.subprocess, "run", side_effect=[SimpleNamespace(stdout=value) for value in replies]), self.assertRaisesRegex(ValueError, "index entries"):
                    q.clean_source(root, "a" * 40, "b" * 40, {})


if __name__ == "__main__":
    unittest.main()
