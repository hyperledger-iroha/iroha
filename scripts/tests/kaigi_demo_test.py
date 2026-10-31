#!/usr/bin/env python3
"""Safety checks for the disposable private Kaigi demo."""

from __future__ import annotations

import os
import re
import subprocess
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "kaigi_demo.sh"


def run_script(*args: str, **env: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["bash", str(SCRIPT), *args],
        check=False,
        capture_output=True,
        text=True,
        env={**os.environ, **env},
        timeout=30,
    )


class KaigiDemoSafetyTest(unittest.TestCase):
    def test_script_parses(self) -> None:
        subprocess.run(["bash", "-n", str(SCRIPT)], check=True)

    def test_help_explains_the_environment_without_building(self) -> None:
        result = run_script("--help")

        self.assertEqual(result.returncode, 0, result.stderr)
        for name in ("RUN_DIR", "BASE_API_PORT", "BASE_P2P_PORT", "KEEP_NETWORK"):
            self.assertIn(name, result.stdout)
        self.assertNotIn("set -euo pipefail", result.stdout)

    def test_unknown_arguments_are_rejected(self) -> None:
        result = run_script("--unknown")

        self.assertEqual(result.returncode, 2)
        self.assertIn("RUN_DIR", result.stderr)

    def test_an_existing_run_directory_stops_before_any_build(self) -> None:
        with tempfile.TemporaryDirectory() as run_dir:
            sentinel = Path(run_dir) / "kept"
            sentinel.write_text("kept", encoding="utf-8")

            result = run_script(RUN_DIR=run_dir)

            self.assertEqual(result.returncode, 1)
            self.assertIn("RUN_DIR already exists", result.stderr)
            self.assertNotIn("building", result.stderr)
            self.assertEqual(sentinel.read_text(encoding="utf-8"), "kept")

    def test_script_enters_repository_before_using_relative_paths(self) -> None:
        text = SCRIPT.read_text(encoding="utf-8")

        root = text.index('ROOT="$(cd -- "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"')
        chdir = text.index('cd -- "$ROOT"')
        first_cargo = text.index("cargo build")
        self.assertLess(root, chdir)
        self.assertLess(chdir, first_cargo)

    def test_every_private_lifecycle_step_proves_from_a_secret_file(self) -> None:
        text = SCRIPT.read_text(encoding="utf-8")

        for step in ("create", "join", "record-usage", "leave", "end"):
            calls = re.findall(rf"app kaigi {step} \\\n(.*?)>/dev/null", text, flags=re.S)
            self.assertTrue(calls, step)
            for call in calls:
                self.assertIn('--secret-file "$RUN_DIR/', call, step)

    def test_run_directory_and_participant_config_are_owner_only(self) -> None:
        text = SCRIPT.read_text(encoding="utf-8")

        self.assertIn('chmod 700 "$RUN_DIR"', text)
        self.assertIn("os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600", text)

    def test_network_stops_through_its_own_script_only(self) -> None:
        text = SCRIPT.read_text(encoding="utf-8")

        self.assertIn('"$NETWORK_DIR/stop.sh"', text)
        self.assertIn("trap cleanup EXIT", text)
        for forbidden in ("kill -9", "kill -KILL", "pkill", "killall", "SIGKILL"):
            self.assertNotIn(forbidden, text)

    def test_both_accounts_name_their_fee_payer(self) -> None:
        text = SCRIPT.read_text(encoding="utf-8")

        self.assertIn('--config "$HOST_CONFIG" --fee-payer authority', text)
        self.assertIn('--config "$PARTICIPANT_CONFIG" --fee-payer authority', text)

    def test_accounts_come_from_the_generated_network(self) -> None:
        text = SCRIPT.read_text(encoding="utf-8")

        self.assertIn("s/^client_account: //p", text)
        self.assertIn('"$RUN_DIR/localnet.txt"', text)
        self.assertNotIn("defaults/", text)


if __name__ == "__main__":
    unittest.main()
