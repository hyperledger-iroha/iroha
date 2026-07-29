"""Tests for the guarded Taira Docker Compose wrapper."""

from __future__ import annotations

import os
import stat
import subprocess
import tempfile
import textwrap
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT_PATH = (
    REPO_ROOT
    / "configs"
    / "soranexus"
    / "taira"
    / "taira-validator-compose.sh"
)
COMPOSE_PATH = (
    REPO_ROOT
    / "configs"
    / "soranexus"
    / "taira"
    / "docker-compose.validator.yml"
)


class TairaValidatorComposeScriptTest(unittest.TestCase):
    """Exercise bounded Compose health admission without a Docker daemon."""

    def setUp(self) -> None:
        self.tempdir = tempfile.TemporaryDirectory()
        self.root = Path(self.tempdir.name)
        self.log_path = self.root / "docker.log"
        self.env_path = self.root / "validator.env"
        config = self.root / "config.toml"
        storage = self.root / "storage"
        policy = self.root / "release-policy.norito"
        artifacts = self.root / "artifacts"
        config.write_text("chain = \"test\"\n", encoding="utf-8")
        storage.mkdir()
        policy.write_bytes(b"policy")
        artifacts.mkdir()
        self.env_path.write_text(
            textwrap.dedent(
                f"""\
                TAIRA_CONFIG_PATH={config}
                TAIRA_STORAGE_PATH={storage}
                TAIRA_KAGEMUSHA_RELEASE_POLICY_PATH={policy}
                TAIRA_KAGEMUSHA_ARTIFACT_DIR={artifacts}
                TAIRA_HEALTH_TIMEOUT_SECONDS=37
                """
            ),
            encoding="utf-8",
        )
        bin_dir = self.root / "bin"
        bin_dir.mkdir()
        docker = bin_dir / "docker"
        docker.write_text(
            textwrap.dedent(
                f"""\
                #!/usr/bin/env bash
                set -euo pipefail
                printf '%s\\n' "$*" >>"{self.log_path}"
                if [[ "$*" == *"config --format json"* ]]; then
                    printf '%s\\n' '{{"services":{{"taira-validator":{{"healthcheck":{{"test":["CMD","curl","-fsS","http://127.0.0.1:8080/readyz"]}},"volumes":[{{"target":"/config/config.toml","read_only":true}},{{"target":"/etc/iroha/kagemusha/release-policy.norito","read_only":true}},{{"target":"/var/lib/iroha/kagemusha/v4","read_only":true}}]}}}}}}'
                    exit 0
                fi
                if [[ "$*" == *" up "* && "${{FAKE_COMPOSE_UP_FAIL:-0}}" == "1" ]]; then
                    exit 1
                fi
                exit 0
                """
            ),
            encoding="utf-8",
        )
        docker.chmod(docker.stat().st_mode | stat.S_IXUSR)
        self.base_env = os.environ.copy()
        self.base_env["PATH"] = f"{bin_dir}:{self.base_env['PATH']}"

    def tearDown(self) -> None:
        self.tempdir.cleanup()

    def run_script(
        self, command: str, *, env: dict[str, str] | None = None
    ) -> subprocess.CompletedProcess[str]:
        run_env = self.base_env.copy()
        if env:
            run_env.update(env)
        return subprocess.run(
            [
                str(SCRIPT_PATH),
                "--env-file",
                str(self.env_path),
                "--compose-file",
                str(COMPOSE_PATH),
                command,
            ],
            cwd=REPO_ROOT,
            env=run_env,
            capture_output=True,
            text=True,
            check=False,
        )

    def calls(self) -> list[str]:
        return self.log_path.read_text(encoding="utf-8").splitlines()

    def test_up_validates_and_waits_for_mandatory_health(self) -> None:
        result = self.run_script("up")

        self.assertEqual(result.returncode, 0, result.stderr)
        calls = self.calls()
        self.assertIn("config --format json", calls[0])
        self.assertIn(
            "up -d --wait --wait-timeout 37 taira-validator",
            calls[1],
        )

    def test_failed_health_wait_removes_only_validator_service(self) -> None:
        result = self.run_script(
            "up",
            env={"FAKE_COMPOSE_UP_FAIL": "1"},
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("mandatory /readyz admission", result.stderr)
        self.assertIn(
            "rm --stop --force taira-validator",
            self.calls()[-1],
        )

    def test_restart_force_recreates_and_waits(self) -> None:
        result = self.run_script("restart")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(
            "up -d --force-recreate --wait --wait-timeout 37 taira-validator",
            self.calls()[-1],
        )

    def test_arbitrary_compose_file_is_rejected(self) -> None:
        unreviewed = self.root / "unreviewed.yml"
        unreviewed.write_text("services: {}\n", encoding="utf-8")
        result = subprocess.run(
            [
                str(SCRIPT_PATH),
                "--env-file",
                str(self.env_path),
                "--compose-file",
                str(unreviewed),
                "up",
            ],
            cwd=REPO_ROOT,
            env=self.base_env,
            capture_output=True,
            text=True,
            check=False,
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("reviewed mandatory-offline Compose file", result.stderr)

    def test_resolved_compose_without_exact_readyz_contract_is_rejected(self) -> None:
        docker = self.root / "bin" / "docker"
        docker.write_text(
            textwrap.dedent(
                f"""\
                #!/usr/bin/env bash
                set -euo pipefail
                printf '%s\\n' "$*" >>"{self.log_path}"
                if [[ "$*" == *"config --format json"* ]]; then
                    printf '%s\\n' '{{"services":{{"taira-validator":{{"healthcheck":{{"test":["CMD","curl","-fsS","http://127.0.0.1:8080/livez"]}},"volumes":[]}}}}}}'
                    exit 0
                fi
                exit 0
                """
            ),
            encoding="utf-8",
        )
        docker.chmod(docker.stat().st_mode | stat.S_IXUSR)

        result = self.run_script("up")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("exact mandatory /readyz healthcheck", result.stderr)


if __name__ == "__main__":
    unittest.main()
