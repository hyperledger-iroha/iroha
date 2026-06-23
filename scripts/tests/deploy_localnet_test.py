"""Static safety tests for scripts/deploy_localnet.sh."""

from __future__ import annotations

import subprocess
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "deploy_localnet.sh"


class DeployLocalnetShellSafetyTest(unittest.TestCase):
    def test_script_parses(self) -> None:
        subprocess.run(["bash", "-n", str(SCRIPT)], check=True)

    def test_force_cleanup_uses_guarded_pid_ownership_checks(self) -> None:
        text = SCRIPT.read_text(encoding="utf-8")
        self.assertIn("pid_matches_localnet_peer()", text)
        self.assertIn("pid_is_running()", text)
        self.assertIn("command -v ps >/dev/null 2>&1 || return 0", text)
        self.assertIn("stop_existing_localnet()", text)
        self.assertIn('grep -F -- "--config $config_path"', text)
        self.assertIn('kill "$pid" 2>/dev/null || true', text)
        self.assertNotIn("kill -0", text)
        self.assertNotIn('kill -9 "$pid"', text)
        self.assertNotIn("kill -KILL", text)
        self.assertNotIn("&& ./stop.sh 2>/dev/null", text)
        self.assertIn(
            "Stopping existing Iroha peers in $OUT_DIR with guarded pid ownership checks",
            text,
        )
        self.assertIn(
            "Out-dir $OUT_DIR still has live or mismatched pidfiles; not removing it.",
            text,
        )
        self.assertIn(
            "Refusing to remove existing out-dir while localnet peer",
            text,
        )
        self.assertIn(
            "live pid $pid does not match $config_path",
            text,
        )


if __name__ == "__main__":
    unittest.main()
