"""Keep KAGEMUSHA package entry points free of pre-release facade aliases.

Run with Python 3.11+ from a source checkout; no native artifacts, environment
variables, or network access are required. These checks only read source files.
"""

from __future__ import annotations

import ast
import json
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


class KagemushaPackageSurfaceTests(unittest.TestCase):
    """Pin the unversioned product facade without removing V1 wire types."""

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
                "browser": "./dist/kagemusha.js",
                "import": "./dist/kagemusha.js",
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
