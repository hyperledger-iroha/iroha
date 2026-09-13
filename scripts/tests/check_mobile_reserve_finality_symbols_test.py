"""Exercise the real shell symbol checker with controlled symbol-tool output.

These are release-gate unit tests, not compiled native-artifact qualification.
"""

from pathlib import Path
import ast
import json
import re
import subprocess
import unittest


ROOT = Path(__file__).resolve().parents[2]
SOURCE = (ROOT / "scripts/check_mobile_sdk_artifacts.sh").read_text()
DECLARATIONS = SOURCE.split("KAGEMUSHA_C_SYMBOLS=(\n", 1)[1].split("\ncheck_source_contract()", 1)[0]
DECLARATIONS = "KAGEMUSHA_C_SYMBOLS=(\n" + DECLARATIONS
FUNCTION = "check_binary_symbols() {" + SOURCE.split("check_binary_symbols() {", 1)[1].split("\ncheck_apple()", 1)[0]


def symbols(name: str) -> list[str]:
    match = re.search(rf"^{name}=\(\n(.*?)\n\)", DECLARATIONS, re.MULTILINE | re.DOTALL)
    assert match is not None
    return match.group(1).split()


class ReserveFinalitySymbolTests(unittest.TestCase):
    def test_apple_builder_manifest_symbols_match_validator(self) -> None:
        builder = (ROOT / "scripts/build_norito_xcframework.sh").read_text()
        inventories = re.findall(r'"required_symbols":\s*(\[.*?\])', builder, re.DOTALL)
        self.assertEqual(len(inventories), 1)
        # Parse the exact JSON emitted by the shell heredoc, without executing
        # the builder. Shell syntax checking cannot find a missing JSON comma.
        emitted = json.loads(inventories[0])
        syntax = ast.parse((ROOT / "scripts/validate_norito_bridge_xcframework.py").read_text())
        expected = [ast.literal_eval(node.value) for node in syntax.body
                    if isinstance(node, ast.Assign)
                    and any(isinstance(target, ast.Name) and target.id == "EXPECTED_REQUIRED_SYMBOLS"
                            for target in node.targets)]
        self.assertEqual(len(expected), 1)
        self.assertEqual(emitted, expected[0])
        self.assertEqual(len(emitted), len(set(emitted)))

    def check(self, mode: str, missing: str | None = None) -> subprocess.CompletedProcess[str]:
        exported = symbols("KAGEMUSHA_C_SYMBOLS") + symbols("REQUIRED_PROTOCOL_C_SYMBOLS")
        if mode == "elf":
            exported += symbols("RESERVE_FINALITY_JNI_SYMBOLS")
        if missing is not None:
            self.assertIn(missing, exported)
            exported.remove(missing)
        # The checked function is copied verbatim. Only nm output is synthetic.
        script = '''set -euo pipefail
FAILURES=0
TEST_SYMBOLS="$1"
fail() { printf '%s\\n' "$*" >&2; FAILURES=$((FAILURES + 1)); }
nm() { printf '%s\\n' "$TEST_SYMBOLS"; }
''' + DECLARATIONS + "\n" + FUNCTION + '''
check_binary_symbols test-only-library test-only-inventory "$2"
[[ "$FAILURES" -eq 0 ]]
'''
        return subprocess.run(["bash", "-c", script, "symbol-unit-test", "\n".join(exported), mode],
                              capture_output=True, text=True, check=False)

    def test_complete_symbol_cohort_passes_for_both_platforms(self) -> None:
        for mode in ("apple", "elf"):
            with self.subTest(mode=mode):
                result = self.check(mode)
                self.assertEqual(result.returncode, 0, result.stderr)

    def test_every_missing_finality_endpoint_is_rejected(self) -> None:
        c_exports = ["connect_norito_kagemusha_reserve_finality_hint_v1",
                     "connect_norito_kagemusha_reserve_finality_verify_v1",
                     "connect_norito_kagemusha_top_up_signed_request_validate_v1"]
        for mode in ("apple", "elf"):
            required = c_exports + (symbols("RESERVE_FINALITY_JNI_SYMBOLS") if mode == "elf" else [])
            for missing in required:
                with self.subTest(mode=mode, missing=missing):
                    result = self.check(mode, missing)
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn("is missing " + missing, result.stderr)


if __name__ == "__main__":
    unittest.main()
