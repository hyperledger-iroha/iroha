"""Static safety checks for scripts/android_sample_env.sh."""

from __future__ import annotations

import subprocess
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "android_sample_env.sh"


class AndroidSampleEnvSafetyTest(unittest.TestCase):
    def test_script_parses(self) -> None:
        subprocess.run(["bash", "-n", str(SCRIPT)], check=True)

    def test_background_liveness_uses_ps_without_sigkill(self) -> None:
        text = SCRIPT.read_text(encoding="utf-8")

        self.assertIn("pid_is_running()", text)
        self.assertIn("pid_is_own_background_job()", text)
        self.assertIn("if ! command -v ps >/dev/null 2>&1; then", text)
        self.assertIn("return 0", text)
        self.assertIn('ps -p "$pid" -o pid=', text)
        self.assertIn("jobs -pr 2>/dev/null || true", text)
        self.assertIn(
            'if pid_is_own_background_job "${TORII_PID}" && pid_is_running "${TORII_PID}"; then',
            text,
        )
        self.assertIn(
            'if pid_is_own_background_job "${HANDOFF_PID}" && pid_is_running "${HANDOFF_PID}"; then',
            text,
        )
        self.assertIn(
            'if ! pid_is_own_background_job "${TORII_PID}" || ! pid_is_running "${TORII_PID}"; then',
            text,
        )
        self.assertIn('kill "${TORII_PID}" >/dev/null 2>&1 || true', text)
        self.assertIn('kill "${HANDOFF_PID}" >/dev/null 2>&1 || true', text)
        self.assertNotIn("kill -0", text)
        self.assertNotIn("kill -9", text)
        self.assertNotIn("kill -KILL", text)
        self.assertNotIn("pkill", text)
        self.assertNotIn("killall", text)
        self.assertNotIn("SIGKILL", text)


if __name__ == "__main__":
    unittest.main()
