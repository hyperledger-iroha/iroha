"""Safety checks for public sandbox output and private runtime file custody."""

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

    def make_workspace(self, directory: str) -> tuple[Path, dict[str, str]]:
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
        return workspace, {
            **os.environ,
            "MOCHI_WORKSPACE_ROOT": str(workspace),
            "MOCHI_PYTHON": sys.executable,
        }

    def test_env_emits_only_public_metadata_and_private_file_reference(self) -> None:
        with tempfile.TemporaryDirectory(prefix="mochi app ") as directory:
            workspace, environment = self.make_workspace(directory)
            env_file = workspace / ".env.local"
            sentinel = "test-only-signer-sentinel-DO-NOT-OUTPUT"
            # Malformed dotenv proves metadata output never parses secret contents.
            env_file.write_text('IROHA_PRIVATE_KEY="' + sentinel + '\n', encoding="utf-8")
            env_file.chmod(0o600)
            result = subprocess.run(
                ["bash", "-x", str(SCRIPT), "env"],
                check=True,
                capture_output=True,
                text=True,
                env=environment,
                timeout=10,
            )
            self.assertIn("export IROHA_CHAIN_ID=mochi-local", result.stdout)
            self.assertIn("export IROHA_ACCOUNT_ID=alice", result.stdout)
            self.assertIn(f"export IROHA_ENV_FILE='{env_file}'", result.stdout)
            self.assertNotIn("IROHA_PRIVATE_KEY", result.stdout + result.stderr)
            self.assertNotIn(sentinel, result.stdout + result.stderr)
            self.assertEqual(env_file.read_text(), 'IROHA_PRIVATE_KEY="' + sentinel + '\n')

    @unittest.skipUnless(os.name == "posix", "Unix runtime custody")
    def test_env_rejects_unsafe_runtime_file_without_partial_exports(self) -> None:
        for kind in ("missing", "public", "symlink", "hardlink", "fifo", "directory"):
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as directory:
                workspace, environment = self.make_workspace(directory)
                env_file = workspace / ".env.local"
                sentinel = "test-only-unsafe-signer-sentinel"
                if kind in ("public", "hardlink", "symlink"):
                    backing = workspace / "fixture-private-input"
                    backing.write_text(sentinel, encoding="utf-8")
                    backing.chmod(0o600)
                    if kind == "hardlink":
                        os.link(backing, env_file)
                    elif kind == "symlink":
                        env_file.symlink_to(backing)
                    else:
                        backing.rename(env_file)
                        env_file.chmod(0o644)
                elif kind == "fifo":
                    os.mkfifo(env_file, 0o600)
                elif kind == "directory":
                    env_file.mkdir(mode=0o700)
                result = subprocess.run(
                    ["bash", str(SCRIPT), "env"],
                    capture_output=True,
                    text=True,
                    env=environment,
                    timeout=10,
                )
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(result.stdout, "")
                self.assertIn("owner-owned 0600", result.stderr)
                self.assertNotIn(sentinel, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
