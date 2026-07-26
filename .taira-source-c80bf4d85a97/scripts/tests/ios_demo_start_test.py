"""Safety and regression tests for scripts/ios_demo/start.sh."""

from __future__ import annotations

import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "ios_demo" / "start.sh"


class IosDemoStartSafetyTest(unittest.TestCase):
    def test_script_parses(self) -> None:
        subprocess.run(["bash", "-n", str(SCRIPT)], check=True)

    def test_value_options_reject_missing_arguments(self) -> None:
        for option in (
            "--config",
            "--artifacts",
            "--artifacts-dir",
            "--telemetry-profile",
            "--profile",
        ):
            with self.subTest(option=option):
                result = subprocess.run(
                    ["bash", str(SCRIPT), option],
                    check=False,
                    capture_output=True,
                    text=True,
                )

                self.assertEqual(result.returncode, 2)
                self.assertIn(f"{option} requires a value", result.stderr)
                self.assertNotIn("unbound variable", result.stderr)

    def test_state_fields_and_credentials_flow_without_real_cargo(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            artifacts = root / "artifacts"
            fake_bin = root / "bin"
            fake_bin.mkdir()
            config = root / "accounts.json"
            config.write_text("{}\n", encoding="utf-8")

            node_log = root / "node.log"
            cargo_marker = root / "fake-cargo-ran"
            cargo_args = root / "cargo.args"
            curl_args = root / "curl.args"
            state_fixture = root / "state-fixture.json"
            state_file = artifacts / "ios_demo_state.json"
            node_log.write_text("node output\n", encoding="utf-8")
            state_fixture.write_text(
                json.dumps(
                    {
                        "torii_url": "http://127.0.0.1:8080",
                        "metrics_url": "http://127.0.0.1:8080/metrics",
                        "stdout_log": str(node_log),
                        "accounts": [
                            {
                                "account_id": "alice",
                                "public_key": "alice-public",
                                "private_key": "alice-private",
                            },
                            {"account_id": "bob", "public_key": "bob-public"},
                        ],
                    }
                ),
                encoding="utf-8",
            )

            # Install the ready-state fixture synchronously while the launcher
            # clears stale artifacts, before the fake background process starts.
            fake_rm = fake_bin / "rm"
            fake_rm.write_text(
                """#!/usr/bin/env bash
set -euo pipefail
command -p rm "$@"
command -p cp "${FAKE_STATE_FIXTURE:?}" "${FAKE_STATE_FILE:?}"
""",
                encoding="utf-8",
            )
            fake_rm.chmod(0o755)

            fake_cargo = fake_bin / "cargo"
            fake_cargo.write_text(
                """#!/usr/bin/env bash
set -euo pipefail
printf 'fake cargo invoked\\n' > "${FAKE_CARGO_MARKER:?}"
printf '%s\\n' "$*" > "${FAKE_CARGO_ARGS:?}"
sleep 1
""",
                encoding="utf-8",
            )
            fake_cargo.chmod(0o755)

            fake_curl = fake_bin / "curl"
            fake_curl.write_text(
                """#!/usr/bin/env bash
set -euo pipefail
printf '%s\\n' "$*" > "${FAKE_CURL_ARGS:?}"
output=""
while [[ $# -gt 0 ]]; do
    if [[ "$1" == "-o" ]]; then
        output="$2"
        break
    fi
    shift
done
[[ -n "$output" ]]
printf 'demo_metric 1\\n' > "$output"
""",
                encoding="utf-8",
            )
            fake_curl.chmod(0o755)

            env = os.environ.copy()
            env.update(
                {
                    "FAKE_CARGO_ARGS": str(cargo_args),
                    "FAKE_CARGO_MARKER": str(cargo_marker),
                    "FAKE_CURL_ARGS": str(curl_args),
                    "FAKE_STATE_FILE": str(state_file),
                    "FAKE_STATE_FIXTURE": str(state_fixture),
                    "PATH": f"{fake_bin}{os.pathsep}{env['PATH']}",
                }
            )
            result = subprocess.run(
                [
                    "bash",
                    str(SCRIPT),
                    "--config",
                    str(config),
                    "--artifacts",
                    str(artifacts),
                    "--telemetry-profile",
                    "test-profile",
                    "--exit-after-ready",
                ],
                check=False,
                capture_output=True,
                text=True,
                env=env,
                timeout=10,
            )

            runner_log = artifacts / "ios_demo_runner.log"
            runner_diagnostics = (
                runner_log.read_text(encoding="utf-8")
                if runner_log.is_file()
                else "runner log was not created"
            )
            self.assertEqual(
                result.returncode,
                0,
                f"{result.stderr}\nrunner log:\n{runner_diagnostics}",
            )
            self.assertTrue(cargo_marker.is_file())
            self.assertIn(
                f"--state {state_file}", cargo_args.read_text(encoding="utf-8")
            )
            self.assertIn(
                "[ios-demo] Torii URL: http://127.0.0.1:8080", result.stdout
            )
            self.assertIn(
                "http://127.0.0.1:8080/metrics",
                curl_args.read_text(encoding="utf-8"),
            )
            self.assertEqual(
                (artifacts / "metrics.prom").read_text(encoding="utf-8"),
                "demo_metric 1\n",
            )
            self.assertEqual(
                (artifacts / "torii.log").read_text(encoding="utf-8"),
                "node output\n",
            )
            self.assertEqual(
                json.loads((artifacts / "torii.jwt").read_text(encoding="utf-8")),
                {
                    "accounts": [
                        {
                            "account_id": "alice",
                            "public_key": "alice-public",
                            "private_key": "alice-private",
                        },
                        {"account_id": "bob", "public_key": "bob-public"},
                    ]
                },
            )

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
