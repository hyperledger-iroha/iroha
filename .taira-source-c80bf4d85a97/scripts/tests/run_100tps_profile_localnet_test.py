"""Static safety tests for scripts/run_100tps_profile_localnet.sh."""

from __future__ import annotations

import subprocess
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "run_100tps_profile_localnet.sh"


class Run100TpsProfileLocalnetSafetyTest(unittest.TestCase):
    def test_script_parses(self) -> None:
        subprocess.run(["bash", "-n", str(SCRIPT)], check=True)

    def test_load_wait_uses_ps_liveness_without_sigkill(self) -> None:
        text = SCRIPT.read_text(encoding="utf-8")

        self.assertIn("pid_is_running()", text)
        self.assertIn("wait_for_pid_with_timeout()", text)
        self.assertIn("command -v ps >/dev/null 2>&1 || return 0", text)
        self.assertIn('ps -p "$pid" -o pid=', text)
        self.assertIn("while pid_is_running \"$pid\"; do", text)
        self.assertIn('kill -TERM "$pid" 2>/dev/null || true', text)
        self.assertIn(
            "pid ${pid} is still running after TERM; leaving it visible",
            text,
        )
        self.assertNotIn("kill -0", text)
        self.assertNotIn("kill -9", text)
        self.assertNotIn("kill -KILL", text)


if __name__ == "__main__":
    unittest.main()
