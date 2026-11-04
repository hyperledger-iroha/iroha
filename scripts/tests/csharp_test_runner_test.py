#!/usr/bin/env python3
"""Exercise C# CI launchers' SDK selection and Microsoft.Testing.Platform dispatch."""

from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


REPO_ROOT = Path(__file__).resolve().parents[2]


class CsharpTestRunnerTests(unittest.TestCase):
    """The real shell launchers must resolve the canonical SDK and forward filters."""

    def run_launcher(self, privacy: bool, version: str) -> tuple[subprocess.CompletedProcess[str], list[dict]]:
        """Run a launcher with recorded SDK calls and a synthetic native preflight."""
        with tempfile.TemporaryDirectory() as directory:
            # The privacy launcher accepts only physically canonical (`pwd -P`) native paths;
            # resolve symlinked temporary roots such as macOS `/var` -> `/private/var`.
            root = Path(directory).resolve()
            (root / "csharp").mkdir()
            (root / "csharp/global.json").write_bytes((REPO_ROOT / "csharp/global.json").read_bytes())
            (root / "scripts").mkdir()
            (root / "scripts/check_native_sdk_artifact.py").write_text(
                "import os, pathlib\npathlib.Path(os.environ['PREFLIGHT']).touch()\n",
                encoding="utf-8",
            )
            native = root / "native"
            native.mkdir()
            calls = root / "calls.jsonl"
            dotnet = root / "dotnet"
            dotnet.write_text(
                f"#!{sys.executable}\n"
                "import json, os, pathlib, sys\n"
                "pin = json.loads(pathlib.Path('global.json').read_text())\n"
                "assert pin['test']['runner'] == 'Microsoft.Testing.Platform'\n"
                "if os.environ['PRIVACY'] == '1':\n"
                "    assert pathlib.Path(os.environ['PREFLIGHT']).exists()\n"
                "with open(os.environ['CALLS'], 'a') as stream:\n"
                "    stream.write(json.dumps({'args':sys.argv[1:], 'pin':pin}) + '\\n')\n"
                "if sys.argv[1:] == ['--version']:\n"
                "    print(os.environ['SDK_VERSION'])\n"
                "else:\n"
                "    assert sys.argv[1:3] == ['test', '--project']\n"
                "    assert sys.argv[3].startswith('tests/')\n",
                encoding="utf-8",
            )
            dotnet.chmod(0o700)
            environment = {
                **os.environ,
                "CALLS": str(calls),
                "SDK_VERSION": version,
                "PREFLIGHT": str(root / "preflight"),
                "PRIVACY": "1" if privacy else "0",
                "PRIVACY_CSHARP_SDK_ROOT": str(root),
                "PRIVACY_CSHARP_DOTNET_BIN": str(dotnet),
                "PRIVACY_CSHARP_PYTHON_BIN": sys.executable,
                "PRIVACY_CSHARP_NATIVE_ARTIFACT": str(native / "libconnect_norito_bridge.so"),
                "PRIVACY_CSHARP_NATIVE_MANIFEST": str(native / "native-sdk-abi33-csharp.json"),
                "IROHA_REQUIRE_PRIVACY_EXACT12_NATIVE": "1",
                "LD_LIBRARY_PATH": str(native),
                "SORAFS_PIN_REGISTER_CSHARP_SDK_ROOT": str(root),
                "SORAFS_PIN_REGISTER_DOTNET_BIN": str(dotnet),
            }
            script = "check_privacy_csharp_sdk.sh" if privacy else "check_sorafs_pin_register_csharp_sdk.sh"
            result = subprocess.run(
                [str(REPO_ROOT / "ci" / script)],
                cwd=root,
                env=environment,
                text=True,
                capture_output=True,
                check=False,
            )
            records = [json.loads(line) for line in calls.read_text().splitlines()] if calls.exists() else []
            return result, records

    def test_launchers_use_canonical_runner_and_execute_requested_filters(self) -> None:
        version = json.loads((REPO_ROOT / "csharp/global.json").read_text())["sdk"]["version"]
        for privacy in (False, True):
            with self.subTest(privacy=privacy):
                result, calls = self.run_launcher(privacy, version)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                tests = calls[1:]
                self.assertEqual(len(tests), 5 if privacy else 1)
                for call in tests:
                    self.assertEqual(call["pin"]["sdk"]["version"], version)
                    self.assertIn("--filter-class" if privacy else "--filter-method", call["args"])
                if privacy:
                    self.assertIn("Hyperledger.Iroha.Sdk.Tests.PrivacyNativeTests", tests[0]["args"])
                else:
                    self.assertIn("*RegisterSoraFsPinManifestAsync*", tests[0]["args"])

    def test_launchers_reject_retired_sdk_before_testing(self) -> None:
        for privacy in (False, True):
            with self.subTest(privacy=privacy):
                result, calls = self.run_launcher(privacy, "8.0.419")
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("10.0.x", result.stderr)
                self.assertEqual([call["args"] for call in calls], [["--version"]])


if __name__ == "__main__":
    unittest.main()
