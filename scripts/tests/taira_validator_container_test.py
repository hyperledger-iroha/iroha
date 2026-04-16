"""Tests for the Taira container wrapper script."""

from __future__ import annotations

import os
import stat
import subprocess
import tempfile
import textwrap
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT_PATH = REPO_ROOT / "configs" / "soranexus" / "taira" / "taira-validator-container.sh"


class TairaValidatorContainerScriptTest(unittest.TestCase):
    """Validate the host-side Docker wrapper without talking to a real daemon."""

    def setUp(self) -> None:
        self.tempdir = tempfile.TemporaryDirectory()
        self.root = Path(self.tempdir.name)
        self.config_path = self.root / "config.toml"
        self.config_path.write_text("chain = \"test\"\n", encoding="utf-8")
        self.storage_path = self.root / "storage"
        self.storage_path.mkdir()
        self.genesis_path = self.root / "genesis.json"
        self.genesis_path.write_text("{}\n", encoding="utf-8")
        self.sites_path = self.root / "sorafs_sites.json"
        self.sites_path.write_text("{}\n", encoding="utf-8")
        self.env_file = self.root / "validator.env"
        self.env_file.write_text(
            textwrap.dedent(
                f"""\
                TAIRA_CONTAINER_NAME=test-validator
                TAIRA_IMAGE=example/taira:test
                TAIRA_CONFIG_PATH={self.config_path}
                TAIRA_STORAGE_PATH={self.storage_path}
                TAIRA_P2P_PORT=1447
                TAIRA_TORII_PORT=19080
                TAIRA_RUST_LOG=debug
                TAIRA_GENESIS_PATH={self.genesis_path}
                TAIRA_SORAFS_SITE_BINDINGS_PATH={self.sites_path}
                """
            ),
            encoding="utf-8",
        )

        self.bin_dir = self.root / "bin"
        self.bin_dir.mkdir()
        self.log_path = self.root / "docker.log"
        docker_path = self.bin_dir / "docker"
        docker_path.write_text(
            textwrap.dedent(
                f"""\
                #!/usr/bin/env bash
                set -euo pipefail
                printf '%s\\n' "$*" >> "{self.log_path}"
                if [[ "${{1:-}}" == "image" && "${{2:-}}" == "inspect" ]]; then
                    exit "${{FAKE_DOCKER_IMAGE_INSPECT_EXIT:-1}}"
                fi
                if [[ "${{1:-}}" == "container" && "${{2:-}}" == "inspect" ]]; then
                    exit "${{FAKE_DOCKER_CONTAINER_INSPECT_EXIT:-1}}"
                fi
                exit 0
                """
            ),
            encoding="utf-8",
        )
        docker_path.chmod(docker_path.stat().st_mode | stat.S_IXUSR)

        self.base_env = os.environ.copy()
        self.base_env["PATH"] = f"{self.bin_dir}:{self.base_env['PATH']}"

    def tearDown(self) -> None:
        self.tempdir.cleanup()

    def run_script(self, *args: str, env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
        run_env = self.base_env.copy()
        if env:
            run_env.update(env)
        return subprocess.run(
            [str(SCRIPT_PATH), "--env-file", str(self.env_file), *args],
            cwd=REPO_ROOT,
            env=run_env,
            capture_output=True,
            text=True,
            check=False,
        )

    def read_docker_log(self) -> list[str]:
        if not self.log_path.exists():
            return []
        return self.log_path.read_text(encoding="utf-8").splitlines()

    def test_config_prints_expected_run_command(self) -> None:
        result = self.run_script("config")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("docker run -d", result.stdout)
        self.assertIn("--name test-validator", result.stdout)
        self.assertIn("-p 1447:1337", result.stdout)
        self.assertIn("-p 19080:8080", result.stdout)
        self.assertIn("IROHA_TAIRA_GENESIS=/config/genesis.json", result.stdout)
        self.assertIn("IROHA_SORAFS_SITE_BINDINGS_FILE=/config/sorafs_sites.json", result.stdout)
        self.assertIn("example/taira:test", result.stdout)

    def test_up_pulls_when_image_missing_then_runs_container(self) -> None:
        result = self.run_script("up")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            self.read_docker_log(),
            [
                "image inspect example/taira:test",
                "pull example/taira:test",
                "container inspect test-validator",
                (
                    "run -d --name test-validator --restart unless-stopped --init "
                    "-e RUST_LOG=debug -p 1447:1337 -p 19080:8080 "
                    f"-v {self.config_path}:/config/config.toml:ro "
                    f"-v {self.storage_path}:/storage "
                    "-e IROHA_TAIRA_GENESIS=/config/genesis.json "
                    f"-v {self.genesis_path}:/config/genesis.json:ro "
                    "-e IROHA_SORAFS_SITE_BINDINGS_FILE=/config/sorafs_sites.json "
                    f"-v {self.sites_path}:/config/sorafs_sites.json:ro "
                    "example/taira:test"
                ),
            ],
        )

    def test_up_recreates_existing_container(self) -> None:
        result = self.run_script(
            "up",
            env={
                "FAKE_DOCKER_IMAGE_INSPECT_EXIT": "0",
                "FAKE_DOCKER_CONTAINER_INSPECT_EXIT": "0",
            },
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("rm -f test-validator", "\n".join(self.read_docker_log()))

    def test_missing_config_fails_fast(self) -> None:
        self.config_path.unlink()
        result = self.run_script("up")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing Taira config", result.stderr)


if __name__ == "__main__":
    unittest.main()
