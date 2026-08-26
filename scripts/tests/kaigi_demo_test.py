"""Static safety checks for scripts/kaigi_demo.sh."""

from __future__ import annotations

import subprocess
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "kaigi_demo.sh"


class KaigiDemoSafetyTest(unittest.TestCase):
    def test_script_parses(self) -> None:
        subprocess.run(["bash", "-n", str(SCRIPT)], check=True)

    def test_script_enters_repository_before_using_relative_paths(self) -> None:
        text = SCRIPT.read_text(encoding="utf-8")

        root = text.index('ROOT="$(cd -- "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"')
        chdir = text.index('cd -- "$ROOT"')
        first_cargo = text.index("cargo run")
        self.assertLess(root, chdir)
        self.assertLess(chdir, first_cargo)

    def test_torii_override_is_used_for_both_readiness_and_cli(self) -> None:
        text = SCRIPT.read_text(encoding="utf-8")

        self.assertIn('TORII_STATUS_URL="${TORII_URL%/}/status"', text)
        self.assertIn('curl -sf "$TORII_STATUS_URL"', text)
        self.assertIn('--torii-url "$TORII_URL"', text)
        self.assertNotIn('curl -sf "$TORII_URL/status"', text)

    def test_background_liveness_uses_owned_job_and_ps_without_sigkill(self) -> None:
        text = SCRIPT.read_text(encoding="utf-8")

        self.assertIn("pid_is_running()", text)
        self.assertIn("pid_is_own_background_job()", text)
        self.assertIn("if ! command -v ps >/dev/null 2>&1; then", text)
        self.assertIn("return 0", text)
        self.assertIn('ps -p "$pid" -o pid=', text)
        self.assertIn("jobs -pr 2>/dev/null || true", text)
        self.assertIn(
            'pid_is_own_background_job "$NODE_PID" && pid_is_running "$NODE_PID"',
            text,
        )
        self.assertIn(
            'if ! pid_is_own_background_job "$NODE_PID" || ! pid_is_running "$NODE_PID"; then',
            text,
        )
        self.assertIn('kill "$NODE_PID" >/dev/null 2>&1 || true', text)
        self.assertNotIn("kill -0", text)
        self.assertNotIn("kill -9", text)
        self.assertNotIn("kill -KILL", text)
        self.assertNotIn("pkill", text)
        self.assertNotIn("killall", text)
        self.assertNotIn("SIGKILL", text)


if __name__ == "__main__":
    unittest.main()
