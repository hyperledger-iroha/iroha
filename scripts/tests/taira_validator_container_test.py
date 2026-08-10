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
COMPOSE_PATH = (
    REPO_ROOT / "configs" / "soranexus" / "taira" / "docker-compose.validator.yml"
)
BARE_METAL_SERVICE_PATH = (
    REPO_ROOT / "configs" / "soranexus" / "taira" / "taira-irohad.service"
)
BARE_METAL_ENV_PATH = (
    REPO_ROOT / "configs" / "soranexus" / "taira" / "taira-irohad.env.example"
)
IMAGE_REFERENCE = f"example/taira@sha256:{'a' * 64}"


class TairaValidatorContainerScriptTest(unittest.TestCase):
    """Validate the host-side Docker wrapper without talking to a real daemon."""

    def setUp(self) -> None:
        self.tempdir = tempfile.TemporaryDirectory()
        self.root = Path(self.tempdir.name).resolve()
        self.config_bundle_path = self.root / "validator-bundle"
        self.config_bundle_path.mkdir()
        self.config_path = self.config_bundle_path / "config.toml"
        self.config_path.write_text(
            textwrap.dedent(
                """\
                chain = "test"

                [sorafs.gateway.site_bindings]
                path = "/config/sorafs_sites.json"
                max_bytes = 1048576
                max_sites = 1024
                """
            ),
            encoding="utf-8",
        )
        self.runtime_path = self.config_bundle_path / "runtime"
        self.runtime_path.mkdir()
        (self.runtime_path / "onboarding-signer.key").write_text(
            "onboarding-key\n", encoding="utf-8"
        )
        (self.runtime_path / "faucet-signer.key").write_text(
            "faucet-key\n", encoding="utf-8"
        )
        self.manifest_path = self.config_bundle_path / "manifests"
        self.manifest_path.mkdir()
        (self.manifest_path / "governance.manifest.json").write_text(
            "{}\n", encoding="utf-8"
        )
        self.admission_path = self.config_bundle_path / "sorafs_admission"
        self.admission_path.mkdir()
        self.storage_path = self.root / "storage"
        self.storage_path.mkdir()
        self.genesis_path = self.root / "genesis.json"
        self.genesis_path.write_text("{}\n", encoding="utf-8")
        self.signed_genesis_path = self.root / "genesis.signed.nrt"
        self.signed_genesis_path.write_bytes(b"norito")
        self.sites_path = self.root / "sorafs_sites.json"
        self.sites_path.write_text(
            '{\n  "version": 1,\n  "sites": []\n}\n', encoding="utf-8"
        )
        self.env_file = self.root / "validator.env"
        self.env_file.write_text(
            textwrap.dedent(
                f"""\
                TAIRA_CONTAINER_NAME=test-validator
                TAIRA_IMAGE={IMAGE_REFERENCE}
                TAIRA_RUNTIME_PROFILE=production
                TAIRA_CONFIG_BUNDLE_PATH={self.config_bundle_path}
                TAIRA_STORAGE_PATH={self.storage_path}
                TAIRA_P2P_PORT=1447
                TAIRA_TORII_PORT=19080
                TAIRA_RUST_LOG=debug
                TAIRA_EXPOSE_KVM=false
                TAIRA_DOCKER_NETWORK=taira-localnet
                TAIRA_GENESIS_PATH={self.genesis_path}
                TAIRA_SIGNED_GENESIS_PATH={self.signed_genesis_path}
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
        self.assertIn("-p 19080:18080", result.stdout)
        self.assertIn(
            "IROHA_TAIRA_CONFIG=/etc/iroha/taira-validator/config.toml",
            result.stdout,
        )
        self.assertIn("IROHA_INROU_PORTABLE_ACCEL=auto", result.stdout)
        self.assertIn("TAIRA_RUNTIME_PROFILE=production", result.stdout)
        self.assertIn(f"TAIRA_IMAGE_REFERENCE={IMAGE_REFERENCE}", result.stdout)
        self.assertIn("--network taira-localnet", result.stdout)
        self.assertIn(
            f"{self.config_bundle_path}:/etc/iroha/taira-validator:ro",
            result.stdout,
        )
        self.assertNotIn("KAGEMUSHA", result.stdout)
        self.assertIn("IROHA_TAIRA_GENESIS=/config/genesis.json", result.stdout)
        self.assertIn("IROHA_TAIRA_SIGNED_GENESIS=/config/genesis.signed.nrt", result.stdout)
        self.assertNotIn("IROHA_SORAFS_SITE_BINDINGS_FILE", result.stdout)
        self.assertIn(f"{self.sites_path}:/config/sorafs_sites.json:ro", result.stdout)
        self.assertIn(IMAGE_REFERENCE, result.stdout)

    def test_compose_uses_the_same_production_runtime_contract(self) -> None:
        compose = COMPOSE_PATH.read_text(encoding="utf-8")
        self.assertIn(
            "IROHA_TAIRA_CONFIG: /etc/iroha/taira-validator/config.toml",
            compose,
        )
        self.assertIn("TAIRA_RUNTIME_PROFILE: production", compose)
        self.assertIn("TAIRA_IMAGE_REFERENCE: ${TAIRA_IMAGE:?", compose)
        self.assertNotIn("hyperledger/iroha:taira-latest", compose)
        self.assertIn(
            '"${TAIRA_TORII_PORT:-18080}:18080"',
            compose,
        )
        self.assertIn(
            "${TAIRA_CONFIG_BUNDLE_PATH:-/etc/iroha/taira-validator}:"
            "/etc/iroha/taira-validator:ro",
            compose,
        )
        self.assertNotIn("KAGEMUSHA", compose)
        self.assertIn(
            "http://127.0.0.1:18080/status",
            compose,
        )
        self.assertNotIn(":8080", compose)

    def test_bare_metal_service_matches_installed_binary_and_asset_contract(
        self,
    ) -> None:
        service = BARE_METAL_SERVICE_PATH.read_text(encoding="utf-8")
        environment = BARE_METAL_ENV_PATH.read_text(encoding="utf-8")

        self.assertIn(
            "Environment=IROHA_TAIRA_IROHAD_BIN=/usr/local/bin/iroha3d",
            service,
        )
        self.assertIn(
            "IROHA_TAIRA_IROHAD_BIN=/usr/local/bin/iroha3d",
            environment,
        )
        self.assertIn(
            "ExecStart=/usr/bin/env -- ${IROHA_TAIRA_IROHAD_BIN} --sora "
            "--config /etc/iroha/taira-validator/config.toml",
            service,
        )
        self.assertIn(
            "ExecStartPre=/usr/bin/env -- ${IROHA_TAIRA_INROU_PREREQ_CHECK}",
            service,
        )
        self.assertIn(
            "Environment=KURA_STORE_DIR=/var/lib/iroha/taira-validator-1",
            service,
        )
        self.assertIn(
            "KURA_STORE_DIR=/var/lib/iroha/taira-validator-1",
            environment,
        )
        self.assertIn(
            "ReadWritePaths=/var/lib/iroha/taira-validator-1 "
            "/var/lib/iroha/taira-validator",
            service,
        )
        self.assertIn(
            "ExecStartPre=/opt/iroha/scripts/docker_entrypoint.sh "
            "--validate-taira-production-config "
            "/etc/iroha/taira-validator/config.toml",
            service,
        )
        self.assertNotIn("KAGEMUSHA", service)
        self.assertNotIn("IROHA_TAIRA_CONFIG=", environment)
        for required_path in (
            "/var/lib/iroha/taira-validator-1",
            "/etc/iroha/taira-validator/runtime/onboarding-signer.key",
            "/etc/iroha/taira-validator/runtime/faucet-signer.key",
            "/etc/iroha/taira-validator/manifests/governance.manifest.json",
            "/etc/iroha/taira-validator/sorafs_admission",
        ):
            self.assertIn(required_path, service)

    def test_up_pulls_when_image_missing_then_runs_container(self) -> None:
        result = self.run_script("up")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            self.read_docker_log(),
            [
                f"image inspect {IMAGE_REFERENCE}",
                f"pull {IMAGE_REFERENCE}",
                "container inspect test-validator",
                (
                    "run -d --name test-validator --restart unless-stopped --init "
                    "--workdir /etc/iroha/taira-validator "
                    "-e RUST_LOG=debug -e IROHA_INROU_PORTABLE_ACCEL=auto "
                    "-e IROHA_TAIRA_CONFIG=/etc/iroha/taira-validator/config.toml "
                    "-e TAIRA_RUNTIME_PROFILE=production "
                    f"-e TAIRA_IMAGE_REFERENCE={IMAGE_REFERENCE} "
                    "-p 1447:1337 -p 19080:18080 "
                    f"-v {self.config_bundle_path}:/etc/iroha/taira-validator:ro "
                    f"-v {self.storage_path}:/storage "
                    "--network taira-localnet "
                    "-e IROHA_TAIRA_GENESIS=/config/genesis.json "
                    f"-v {self.genesis_path}:/config/genesis.json:ro "
                    "-e IROHA_TAIRA_SIGNED_GENESIS=/config/genesis.signed.nrt "
                    f"-v {self.signed_genesis_path}:/config/genesis.signed.nrt:ro "
                    f"-v {self.sites_path}:/config/sorafs_sites.json:ro "
                    f"{IMAGE_REFERENCE}"
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
        self.assertIn("Taira config", result.stderr)

    def test_missing_explicit_environment_file_fails_before_docker(self) -> None:
        result = subprocess.run(
            [
                str(SCRIPT_PATH),
                "--env-file",
                str(self.root / "missing.env"),
                "up",
            ],
            cwd=REPO_ROOT,
            env=self.base_env,
            capture_output=True,
            text=True,
            check=False,
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Taira environment file", result.stderr)
        self.assertEqual(self.read_docker_log(), [])

    def test_production_rejects_mutable_image_tag_before_docker(self) -> None:
        self.env_file.write_text(
            self.env_file.read_text(encoding="utf-8").replace(
                f"TAIRA_IMAGE={IMAGE_REFERENCE}",
                "TAIRA_IMAGE=example/taira:latest",
            ),
            encoding="utf-8",
        )

        result = self.run_script("up")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("immutable image ID or repository@sha256 digest", result.stderr)
        self.assertEqual(self.read_docker_log(), [])

    def test_down_remains_available_when_image_reference_needs_repair(self) -> None:
        self.env_file.write_text(
            self.env_file.read_text(encoding="utf-8").replace(
                f"TAIRA_IMAGE={IMAGE_REFERENCE}",
                "TAIRA_IMAGE=example/taira:legacy",
            ),
            encoding="utf-8",
        )

        result = self.run_script(
            "down",
            env={"FAKE_DOCKER_CONTAINER_INSPECT_EXIT": "0"},
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            self.read_docker_log(),
            [
                "container inspect test-validator",
                "rm -f test-validator",
            ],
        )

    def test_missing_runtime_sidecar_fails_fast(self) -> None:
        (self.runtime_path / "onboarding-signer.key").unlink()
        result = self.run_script("config")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Taira onboarding signer", result.stderr)

    def test_symlinked_config_bundle_directory_fails_fast(self) -> None:
        bundle_alias = self.root / "bundle-alias"
        bundle_alias.symlink_to(self.config_bundle_path, target_is_directory=True)
        self.env_file.write_text(
            self.env_file.read_text(encoding="utf-8").replace(
                f"TAIRA_CONFIG_BUNDLE_PATH={self.config_bundle_path}",
                f"TAIRA_CONFIG_BUNDLE_PATH={bundle_alias}",
            ),
            encoding="utf-8",
        )
        result = self.run_script("config")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("symlinked Taira config bundle", result.stderr)

    def test_symlinked_runtime_directory_fails_fast(self) -> None:
        for path in self.runtime_path.iterdir():
            path.unlink()
        self.runtime_path.rmdir()
        external_runtime = self.root / "external-runtime"
        external_runtime.mkdir()
        (external_runtime / "onboarding-signer.key").write_text(
            "onboarding-key\n", encoding="utf-8"
        )
        (external_runtime / "faucet-signer.key").write_text(
            "faucet-key\n", encoding="utf-8"
        )
        self.runtime_path.symlink_to(external_runtime, target_is_directory=True)

        result = self.run_script("config")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("symlinked Taira runtime directory", result.stderr)

    def test_explicit_localnet_profile_uses_8080_without_production_sidecars(
        self,
    ) -> None:
        self.env_file.write_text(
            self.env_file.read_text(encoding="utf-8").replace(
                "TAIRA_RUNTIME_PROFILE=production",
                "TAIRA_RUNTIME_PROFILE=localnet",
            ),
            encoding="utf-8",
        )
        for path in (
            self.manifest_path / "governance.manifest.json",
            self.runtime_path / "onboarding-signer.key",
            self.runtime_path / "faucet-signer.key",
        ):
            path.unlink()

        result = self.run_script("config")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("-p 19080:8080", result.stdout)
        self.assertNotIn("KAGEMUSHA", result.stdout)

    def test_unknown_runtime_profile_fails_before_any_docker_action(self) -> None:
        self.env_file.write_text(
            self.env_file.read_text(encoding="utf-8").replace(
                "TAIRA_RUNTIME_PROFILE=production",
                "TAIRA_RUNTIME_PROFILE=legacy",
            ),
            encoding="utf-8",
        )
        result = self.run_script("reset")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "TAIRA_RUNTIME_PROFILE must be exactly production or localnet",
            result.stderr,
        )
        self.assertEqual(self.read_docker_log(), [])

    def test_reset_wipes_validator_storage(self) -> None:
        (self.storage_path / "blocks").write_bytes(b"blocks")
        result = self.run_script("reset")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(list(self.storage_path.iterdir()), [])

    def test_reset_rejects_config_and_state_overlap_before_docker(self) -> None:
        sentinel = self.config_bundle_path / "do-not-delete"
        sentinel.write_text("config\n", encoding="utf-8")
        self.env_file.write_text(
            self.env_file.read_text(encoding="utf-8").replace(
                f"TAIRA_STORAGE_PATH={self.storage_path}",
                f"TAIRA_STORAGE_PATH={self.config_bundle_path}",
            ),
            encoding="utf-8",
        )

        result = self.run_script("reset")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("must not be equal or nested", result.stderr)
        self.assertTrue(sentinel.exists())
        self.assertEqual(self.read_docker_log(), [])

    def test_reset_rejects_descendant_of_system_root_before_docker(self) -> None:
        self.env_file.write_text(
            self.env_file.read_text(encoding="utf-8").replace(
                f"TAIRA_STORAGE_PATH={self.storage_path}",
                "TAIRA_STORAGE_PATH=/etc/taira-reset-must-not-touch",
            ),
            encoding="utf-8",
        )

        result = self.run_script("reset")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("refusing broad system state directory", result.stderr)
        self.assertEqual(self.read_docker_log(), [])

    def test_site_binding_mount_requires_config_backed_path(self) -> None:
        self.config_path.write_text('chain = "test"\n', encoding="utf-8")
        result = self.run_script("config")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("[sorafs.gateway.site_bindings].path", result.stderr)


if __name__ == "__main__":
    unittest.main()
