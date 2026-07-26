"""Static safety checks for scripts/run_local_swarm.sh."""

from __future__ import annotations

import subprocess
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "run_local_swarm.sh"


def _script_text() -> str:
    return SCRIPT.read_text(encoding="utf-8")


def _generated_stop_script_body() -> str:
    text = _script_text()
    marker = 'cat > "$BASE/stop.sh" <<\'EOF\'\n'
    start = text.index(marker) + len(marker)
    end = text.index("\nEOF\n", start)
    return text[start:end] + "\n"


class RunLocalSwarmSafetyTest(unittest.TestCase):
    def test_script_and_generated_stop_script_have_valid_bash_syntax(self) -> None:
        subprocess.run(["bash", "-n", str(SCRIPT)], check=True)
        subprocess.run(
            ["bash", "-n"],
            input=_generated_stop_script_body().encode("utf-8"),
            check=True,
        )

    def test_stop_guidance_uses_guarded_pid_ownership_checks(self) -> None:
        text = _script_text()
        stop_body = _generated_stop_script_body()

        self.assertNotIn("xargs kill", text)
        self.assertNotIn('rm -f "$BASE"/peer*.pid', text)
        self.assertIn("preflight_existing_pidfiles", text)
        self.assertIn("pid_is_running()", text)
        self.assertIn("command -v ps >/dev/null 2>&1 || return 0", text)
        self.assertNotIn("kill -0", text)
        self.assertIn("Refusing to overwrite live local-swarm peer", text)
        self.assertIn("To stop safely: cd $BASE && ./stop.sh", text)
        self.assertIn("pid_matches_peer()", stop_body)
        self.assertIn("pid_is_running()", stop_body)
        self.assertIn("command -v ps >/dev/null 2>&1 || return 0", stop_body)
        self.assertIn('grep -F -- "--config $config"', stop_body)
        self.assertIn("live pid $pid does not match $config", stop_body)
        self.assertIn('kill "$pid" 2>/dev/null || true', stop_body)
        self.assertNotIn('kill -9 "$pid"', stop_body)
        self.assertIn(
            "local-swarm peer $peer_name pid $pid is still running",
            stop_body,
        )


if __name__ == "__main__":
    unittest.main()
