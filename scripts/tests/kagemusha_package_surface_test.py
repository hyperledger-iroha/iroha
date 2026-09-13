"""Keep KAGEMUSHA package entry points free of pre-release facade aliases.

Run with Python 3.11+ from a source checkout; no native artifacts, environment
variables, or network access are required. These checks only read source files.
"""

from __future__ import annotations

import ast
import json
import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


def coordinator_method_inventory(source: str, language: str) -> list[tuple[str, int]]:
    """Read the complete enum, rejecting entries outside its declared syntax."""
    bodies = re.findall(
        r"\benum(?: class)? (?:ConnectNorito)?KagemushaCoreCoordinatorMethodV1"
        r"\b[^{}]*\{([^{}]*)\}", source,
    )
    if len(bodies) != 1:
        raise AssertionError("expected exactly one coordinator method enum")
    body = re.sub(r"//[^\n]*", "", bodies[0])
    if language == "swift":
        body = re.sub(r"\bcase\s+", ",", body)
    patterns = {
        "c": r"CONNECT_NORITO_KAGEMUSHA_CORE_COORDINATOR_([A-Z_]+)_V1\s*=\s*(\d+)",
        "rust": r"([A-Za-z]+)\s*=\s*(\d+)",
        "swift": r"([A-Za-z]+)(?:\s*=\s*(\d+))?",
        "kotlin": r"([A-Z_]+)\((\d+)\)",
    }
    result = []
    code = -1  # Swift's first implicit raw value is zero.
    for entry in body.split(","):
        if not entry.strip():
            continue
        match = re.fullmatch(patterns[language], entry.strip())
        if match is None:
            raise AssertionError(f"unrecognized {language} coordinator enum entry: {entry!r}")
        name, explicit_code = match.groups()
        code = int(explicit_code) if explicit_code is not None else code + 1
        result.append((name, code))
    return result


class KagemushaPackageSurfaceTests(unittest.TestCase):
    """Pin the unversioned product facade without removing V1 wire types."""

    def test_coordinator_methods_match_exact_c_rust_swift_kotlin_and_fixture_inventory(self) -> None:
        # The probe's ten output words are independent of the eleven method codes.
        # Preserve each language's exact public spelling, including Swift's ID.
        names = (
            ("RESERVE_OPERATION_ID", "ReserveOperationId", "reserveOperationID"),
            ("ACCEPT_QUALIFICATION", "AcceptQualification", "acceptQualification"),
            ("ACCEPT_AUTHENTICATED_REPLY", "AcceptAuthenticatedReply", "acceptAuthenticatedReply"),
            ("BEGIN_SENDER_TRANSITION", "BeginSenderTransition", "beginSenderTransition"),
            ("PROVE_PREPARED_SENDER_TRANSITION", "ProvePreparedSenderTransition", "provePreparedSenderTransition"),
            ("BUILD_TERMINAL_ENVELOPE", "BuildTerminalEnvelope", "buildTerminalEnvelope"),
            ("ACCEPT_INSTALLED_TERMINAL", "AcceptInstalledTerminal", "acceptInstalledTerminal"),
            ("RECOVER_SENDER", "RecoverSender", "recoverSender"),
            ("RECOVER_TERMINAL_ENVELOPE", "RecoverTerminalEnvelope", "recoverTerminalEnvelope"),
            ("RELEASE_OUTBOX", "ReleaseOutbox", "releaseOutbox"),
            ("BEGIN_OBSERVATION", "BeginObservation", "beginObservation"),
        )
        contracts = {
            "c": "crates/connect_norito_bridge/include/connect_norito_bridge.h",
            "rust": "crates/connect_norito_bridge/src/kagemusha_core_coordinator_v1.rs",
            "swift": "IrohaSwift/Sources/IrohaSwift/KagemushaCoreCoordinatorFrameV1.swift",
            "kotlin": "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaCoreCoordinatorFrameV1.kt",
        }
        name_columns = {"c": 0, "rust": 1, "swift": 2, "kotlin": 0}
        for language, path in contracts.items():
            with self.subTest(language=language):
                expected = [
                    (row[name_columns[language]], code)
                    for code, row in enumerate(names, start=1)
                ]
                self.assertEqual(
                    coordinator_method_inventory((ROOT / path).read_text(), language), expected,
                )
        fixture = ROOT / "fixtures/offline/kagemusha_core_coordinator_frame_v1.tsv"
        methods = {
            int(line.split("\t")[1]) for line in fixture.read_text().splitlines()
            if line and not line.startswith("#")
        }
        self.assertEqual(methods, set(range(1, 12)))

    def test_coordinator_frame_schema_matches_c_rust_swift_and_shared_fixtures(self) -> None:
        contracts = (
            ("crates/connect_norito_bridge/include/connect_norito_bridge.h",
             r"#define CONNECT_NORITO_KAGEMUSHA_CORE_COORDINATOR_FRAME_VERSION_V1 UINT16_C\((\d+)\)"),
            ("crates/connect_norito_bridge/src/kagemusha_core_coordinator_v1.rs",
             r"pub const KAGEMUSHA_CORE_COORDINATOR_FRAME_VERSION_V1: u16 = (\d+);"),
            ("IrohaSwift/Sources/IrohaSwift/KagemushaCoreCoordinatorFrameV1.swift",
             r"public static let schemaVersion: UInt16 = (\d+)"),
        )
        for path, pattern in contracts:
            with self.subTest(path=path):
                self.assertEqual(re.findall(pattern, (ROOT / path).read_text()), ["2"])
        fixture = ROOT / "fixtures/offline/kagemusha_core_coordinator_frame_v1.tsv"
        rows = 0
        for line in fixture.read_text().splitlines():
            if not line or line.startswith("#"):
                continue
            name, method, request, response = line.split("\t")
            self.assertIn(int(method), range(1, 12))
            for direction, encoded in (("request", request), ("response", response)):
                with self.subTest(name=name, direction=direction):
                    frame = bytes.fromhex(encoded)
                    self.assertEqual(frame[:8], b"IKGMCOR1")
                    self.assertEqual(int.from_bytes(frame[8:10], "little"), 2)
            rows += 1
        self.assertGreaterEqual(rows, 11)

    def test_superseded_facade_files_are_absent(self) -> None:
        pairs = (
            (
                "csharp/src/Hyperledger.Iroha.Sdk/Kagemusha",
                "KagemushaV1.cs",
                "Kagemusha.cs",
            ),
            ("javascript/iroha_js", "kagemusha-v1.d.ts", "kagemusha.d.ts"),
            ("javascript/iroha_js/src", "kagemushaV1.js", "kagemusha.js"),
            ("javascript/iroha_js/test", "kagemushaV1.test.js", "kagemusha.test.js"),
            (
                "python/iroha_python/src/iroha_python",
                "kagemusha_v1.py",
                "kagemusha.py",
            ),
            ("python/iroha_python/tests", "kagemusha_v1_test.py", "kagemusha_test.py"),
        )
        for directory, retired, canonical in pairs:
            with self.subTest(directory=directory):
                retired_path = ROOT / directory / retired
                self.assertFalse(retired_path.exists() or retired_path.is_symlink())
                self.assertTrue((ROOT / directory / canonical).is_file())

    def test_javascript_manifest_publishes_only_the_canonical_subpath(self) -> None:
        package = json.loads(
            (ROOT / "javascript/iroha_js/package.json").read_text(encoding="utf-8")
        )
        self.assertEqual(
            package["exports"]["./kagemusha"],
            {
                "browser": "./dist/public/kagemusha.js",
                "import": "./dist/public/kagemusha.js",
                "types": "./kagemusha.d.ts",
            },
        )
        self.assertEqual(
            package["typesVersions"]["*"]["kagemusha"], ["./kagemusha.d.ts"]
        )
        self.assertIn("kagemusha.d.ts", package["files"])
        for retired in ("kagemusha-v1", "kagemushaV1"):
            self.assertNotIn(retired, json.dumps(package))
        for path in ("src/index.js", "src/browser.js", "index.d.ts", "browser.d.ts"):
            source = (ROOT / "javascript/iroha_js" / path).read_text(encoding="utf-8")
            self.assertIn('export { Kagemusha } from "./kagemusha.js";', source)
            self.assertNotRegex(source, r"\bKagemushaV1\b")

    def test_python_exports_only_the_canonical_facade(self) -> None:
        package_root = ROOT / "python/iroha_python/src/iroha_python"
        module = ast.parse((package_root / "kagemusha.py").read_text(encoding="utf-8"))
        classes = {node.name for node in module.body if isinstance(node, ast.ClassDef)}
        self.assertIn("Kagemusha", classes)
        self.assertNotIn("KagemushaV1", classes)
        exports = [
            ast.literal_eval(node.value)
            for node in module.body
            if isinstance(node, ast.Assign)
            and any(
                isinstance(target, ast.Name) and target.id == "__all__"
                for target in node.targets
            )
        ]
        self.assertEqual(exports, [["Kagemusha"]])
        initializer = (package_root / "__init__.py").read_text(encoding="utf-8")
        self.assertIn("from .kagemusha import Kagemusha", initializer)
        self.assertNotRegex(initializer, r"\bKagemushaV1\b|\bkagemusha_v1\b")

    def test_csharp_exposes_the_canonical_facade_without_a_forwarder(self) -> None:
        facade_root = ROOT / "csharp/src/Hyperledger.Iroha.Sdk/Kagemusha"
        source = (facade_root / "Kagemusha.cs").read_text(encoding="utf-8")
        self.assertRegex(source, r"public static class Kagemusha\s*\{")
        for path in facade_root.glob("*.cs"):
            with self.subTest(path=path.name):
                # The IPM1 profile enum is a real wire-version marker, not a
                # facade. Reject declarations/forwarders without banning it.
                self.assertNotRegex(
                    path.read_text(encoding="utf-8"),
                    r"\b(?:class|struct|record)\s+KagemushaV1\b|\busing\s+KagemushaV1\s*=",
                )


if __name__ == "__main__":
    unittest.main()
