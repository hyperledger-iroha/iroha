"""Static safety tests for scripts/training_script_2.sh."""

from __future__ import annotations

import subprocess
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "training_script_2.sh"


class TrainingScriptLocalnetSafetyTest(unittest.TestCase):
    def test_script_parses(self) -> None:
        subprocess.run(["bash", "-n", str(SCRIPT)], check=True)

    def test_stop_localnet_uses_guarded_pid_ownership_checks(self) -> None:
        text = SCRIPT.read_text(encoding="utf-8")
        self.assertIn("pid_matches_localnet_peer()", text)
        self.assertIn("pid_is_running()", text)
        self.assertIn("command -v ps >/dev/null 2>&1 || return 0", text)
        self.assertIn("stop_localnet()", text)
        self.assertIn('grep -F -- "--config $config_path"', text)
        self.assertIn('kill "$pid" 2>/dev/null || true', text)
        self.assertNotIn("kill -0", text)
        self.assertNotIn('kill -9 "$pid"', text)
        self.assertNotIn("kill -KILL", text)
        self.assertNotIn("&& ./stop.sh", text)
        self.assertNotIn("(cd \"$run_dir\" && ./stop.sh)", text)
        self.assertIn(
            "refusing to remove $run_dir while peer $peer_name pid $pid is still running",
            text,
        )
        self.assertIn(
            "run dir has live or mismatched pidfiles; not regenerating $run_dir",
            text,
        )
        self.assertIn(
            "live pid $pid does not match $config_path",
            text,
        )
        self.assertIn(
            'stop_localnet "$cleanup_run_dir" || true',
            text,
        )


if __name__ == "__main__":
    unittest.main()
