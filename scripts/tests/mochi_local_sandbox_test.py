"""Static safety checks for scripts/mochi_local_sandbox.sh."""

from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
import json
import os
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

    def test_headless_launcher_enables_the_cli_implementation(self) -> None:
        text = SCRIPT.read_text(encoding="utf-8")

        self.assertIn(
            "cargo run -p mochi-ui --features gui --bin mochi -- sandbox serve",
            text,
        )
        self.assertNotIn("cargo run -p mochi-ui --features gui -- sandbox serve", text)
        self.assertNotIn("cargo run -p mochi-ui -- sandbox serve", text)

    def test_python_interpreter_is_explicitly_configurable(self) -> None:
        text = SCRIPT.read_text(encoding="utf-8")

        self.assertIn('PYTHON_BIN="${MOCHI_PYTHON:-python3}"', text)
        self.assertIn('"$PYTHON_BIN" - "$root"', text)
        self.assertIn('pid="$("$PYTHON_BIN" - "$REPO_ROOT"', text)
        self.assertNotIn("python3 - ", text)

    def test_env_reads_private_key_only_from_owner_only_dotenv(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            sandbox = workspace / ".mochi" / "sandbox" / "four-peer-bft"
            sandbox.mkdir(parents=True)
            session = {
                "api_base": "http://127.0.0.1:8080",
                "torii_url": "http://127.0.0.1:8080",
                "chain_id": "mochi-local",
                "mcp_url": "http://127.0.0.1:8080/v1/mcp",
                "account_id": "alice",
            }
            (sandbox / "session.json").write_text(json.dumps(session), encoding="utf-8")
            env_file = workspace / ".env.local"
            env_file.write_text('IROHA_PRIVATE_KEY="private key value"\n', encoding="utf-8")
            os.chmod(env_file, 0o600)

            result = subprocess.run(
                ["bash", str(SCRIPT), "env"],
                check=True,
                capture_output=True,
                text=True,
                env={
                    **os.environ,
                    "MOCHI_WORKSPACE_ROOT": str(workspace),
                    "MOCHI_PYTHON": sys.executable,
                },
            )
            self.assertIn("export IROHA_PRIVATE_KEY='private key value'", result.stdout)
            self.assertNotIn("private_key", (sandbox / "session.json").read_text())


if __name__ == "__main__":
    unittest.main()
