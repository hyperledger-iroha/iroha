"""Static safety checks for scripts/ios_demo/start.sh."""

from __future__ import annotations

import subprocess
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "ios_demo" / "start.sh"


class IosDemoStartSafetyTest(unittest.TestCase):
    def test_script_parses(self) -> None:
        subprocess.run(["bash", "-n", str(SCRIPT)], check=True)

    def test_background_liveness_uses_owned_job_and_ps_without_sigkill(self) -> None:
        text = SCRIPT.read_text(encoding="utf-8")

        self.assertIn("pid_is_running()", text)
        self.assertIn("pid_is_own_background_job()", text)
        self.assertIn("if ! command -v ps >/dev/null 2>&1; then", text)
        self.assertIn("return 0", text)
        self.assertIn('ps -p "$pid" -o pid=', text)
        self.assertIn("jobs -pr 2>/dev/null || true", text)
        self.assertIn(
            'pid_is_own_background_job "${PID}" && pid_is_running "${PID}"',
            text,
        )
        self.assertIn(
            'if ! pid_is_own_background_job "${PID}" || ! pid_is_running "${PID}"; then',
            text,
        )
        self.assertIn('kill "${PID}" >/dev/null 2>&1 || true', text)
        self.assertNotIn("kill -0", text)
        self.assertNotIn("kill -9", text)
        self.assertNotIn("kill -KILL", text)
        self.assertNotIn("pkill", text)
        self.assertNotIn("killall", text)
        self.assertNotIn("SIGKILL", text)


if __name__ == "__main__":
    unittest.main()
