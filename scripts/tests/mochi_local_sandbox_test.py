"""Static safety checks for scripts/mochi_local_sandbox.sh."""

from __future__ import annotations

import subprocess
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "mochi_local_sandbox.sh"


class MochiLocalSandboxSafetyTest(unittest.TestCase):
    def test_script_parses(self) -> None:
        subprocess.run(["bash", "-n", str(SCRIPT)], check=True)

    def test_pidfile_liveness_uses_ps_and_command_ownership(self) -> None:
        text = SCRIPT.read_text(encoding="utf-8")

        self.assertIn("pid_running()", text)
        self.assertIn("pid_matches_sandbox_command()", text)
        self.assertIn("if ! command -v ps >/dev/null 2>&1; then", text)
        self.assertIn('ps -p "$pid" -o pid=', text)
        self.assertIn('ps -p "$pid" -o command=', text)
        self.assertIn('[[ "$command_line" == *"sandbox serve"* ]] || return 1', text)
        self.assertIn('[[ "$command_line" == *"$workspace_root"* ]] || return 1', text)
        self.assertIn("status=\"mismatched-pid\"", text)
        self.assertIn(
            'if ! pid_matches_sandbox_command "$pid" "$workspace_root"; then',
            text,
        )
        self.assertIn('pid_matches_sandbox_command "$session_pid" "$workspace_root"', text)
        self.assertIn("Refusing to reuse live pid", text)
        self.assertIn("Refusing to stop live pid", text)
        self.assertIn('kill -TERM "$pid" 2>/dev/null || true', text)
        self.assertNotIn("kill -0", text)
        self.assertNotIn("kill -9", text)
        self.assertNotIn("kill -KILL", text)
        self.assertNotIn("pkill", text)
        self.assertNotIn("killall", text)
        self.assertNotIn("SIGKILL", text)


if __name__ == "__main__":
    unittest.main()
