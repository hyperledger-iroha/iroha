"""Mutation tests for complete compiler-produced oracle harness admission."""
from copy import deepcopy
import importlib.util
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("oracle", ROOT / "ci/native_prover_oracle.py")
ORACLE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ORACLE)
TARGET = ROOT / "target/qualification/oracle-admission-test"


def messages():
    return [{"reason": "compiler-artifact", "target": {"name": name},
             "profile": {"test": True}, "executable": str(TARGET / name)}
            for name in ORACLE.HARNESSES] + [{"reason": "build-finished", "success": True}]


def encoded(value):
    return "\n".join(json.dumps(message) for message in value)


class OracleAdmissionTests(unittest.TestCase):
    def test_disabled_oracle_mode_or_missing_large_cases_fail(self):
        for name, required in ORACLE.REQUIRED_TESTS.items():
            listing = "\n".join(f"{test}: test" for test in required) + f"\n{ORACLE.TIMING_TEST}: test\ntiming::parser: test\n"
            self.assertEqual(ORACLE.test_inventory(name, listing), required | {"timing::parser"})
            for absent in required:
                with self.subTest(name=name, absent=absent), self.assertRaises(ValueError):
                    ORACLE.test_inventory(name, listing.replace(f"{absent}: test\n", ""))

    def test_partial_empty_ignored_or_repeated_results_fail(self):
        self.assertTrue(ORACLE.complete_result("test result: ok. 45 passed; 0 failed; 0 ignored;", 45))
        for result, expected in [
            ("test result: ok. 0 passed; 0 failed; 0 ignored;", 0),
            ("test result: ok. 37 passed; 0 failed; 8 ignored;", 45),
            ("test result: ok. 37 passed; 0 failed; 0 ignored;", 45),
            ("test result: FAILED. 44 passed; 1 failed; 0 ignored;", 45),
            ("test result: ok. 45 passed; 0 failed; 0 ignored;\n" * 2, 45),
        ]:
            with self.subTest(result=result):
                self.assertFalse(ORACLE.complete_result(result, expected))

    def test_complete_compiler_harnesses_and_correctness_only_execution(self):
        found = ORACLE.compiler_harnesses(encoded(messages()), TARGET)
        self.assertEqual(set(found), set(ORACLE.HARNESSES))
        for path in found.values():
            self.assertEqual(ORACLE.test_command(path), [str(path), "--include-ignored", "--skip", ORACLE.TIMING_TEST, "--test-threads=2"])

    def test_missing_or_duplicate_target_is_not_partial_success(self):
        baseline = messages()
        for index in range(len(ORACLE.HARNESSES)):
            for mutation in (baseline[:index] + baseline[index + 1:], [baseline[index], *baseline]):
                with self.subTest(index=index), self.assertRaises(ValueError):
                    ORACLE.compiler_harnesses(encoded(mutation), TARGET)

    def test_benchmark_non_test_foreign_path_and_failed_build_are_rejected(self):
        for kind in ("benchmark", "non_test", "foreign", "relative", "failed", "unfinished", "duplicate_finish"):
            value = deepcopy(messages())
            if kind == "benchmark": value[0]["target"]["name"] = "kernel_benchmarks"
            elif kind == "non_test": value[0]["profile"]["test"] = False
            elif kind == "foreign": value[0]["executable"] = str(ROOT / "unrelated-binary")
            elif kind == "relative": value[0]["executable"] = "relative-binary"
            elif kind == "failed": value[-1]["success"] = False
            elif kind == "unfinished": value.pop()
            else: value.append(value[-1])
            with self.subTest(kind=kind), self.assertRaises(ValueError):
                ORACLE.compiler_harnesses(encoded(value), TARGET)


if __name__ == "__main__":
    unittest.main()
