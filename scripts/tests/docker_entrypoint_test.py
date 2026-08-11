"""Tests for scripts/docker_entrypoint.sh."""

from __future__ import annotations

import os
import stat
import subprocess
import tempfile
import unittest
from pathlib import Path


ENTRYPOINT_PATH = Path(__file__).resolve().parents[1] / "docker_entrypoint.sh"


class DockerEntrypointTest(unittest.TestCase):
    """Exercises the image entrypoint without requiring a Docker build."""

    def setUp(self) -> None:
        self._tempdir = tempfile.TemporaryDirectory()
        self.addCleanup(self._tempdir.cleanup)
        self.temp_path = Path(self._tempdir.name)
        self.bin_dir = self.temp_path / "bin"
        self.bin_dir.mkdir()
        self.irohad_path = self.bin_dir / "iroha3d"
        self.irohad_path.write_text(
            "#!/usr/bin/env bash\n"
            "printf '%s\\n' \"$0\" \"$@\"\n",
            encoding="utf-8",
        )
        self.irohad_path.chmod(self.irohad_path.stat().st_mode | stat.S_IEXEC)

    def _run(self, *args: str, env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
        full_env = os.environ.copy()
        full_env["PATH"] = f"{self.bin_dir}:{full_env['PATH']}"
        if env:
            full_env.update(env)
        return subprocess.run(
            [str(ENTRYPOINT_PATH), *args],
            capture_output=True,
            text=True,
            env=full_env,
            check=False,
        )

    def test_defaults_to_plain_iroha3d_for_non_taira_profiles(self) -> None:
        result = self._run()

        self.assertEqual(result.returncode, 0)
        self.assertEqual(
            result.stdout.splitlines(),
            [str(self.irohad_path)],
        )

    def test_defaults_to_taira_boot_command(self) -> None:
        config_path = self.temp_path / "config.toml"
        runtime_config_path = self.temp_path / "runtime-config.toml"
        genesis_path = self.temp_path / "genesis.json"
        config_path.write_text("# config\nchain = \"taira\"\n", encoding="utf-8")
        genesis_path.write_text("{}\n", encoding="utf-8")

        result = self._run(
            env={
                "IROHA_IMAGE_CONFIG_PROFILE": "taira",
                "IROHA_TAIRA_CONFIG": str(config_path),
                "IROHA_TAIRA_RUNTIME_CONFIG": str(runtime_config_path),
                "IROHA_TAIRA_GENESIS": str(genesis_path),
                "TAIRA_RUNTIME_PROFILE": "localnet",
            }
        )

        self.assertEqual(result.returncode, 0)
        self.assertEqual(runtime_config_path.read_text(encoding="utf-8"), config_path.read_text(encoding="utf-8"))
        self.assertEqual(
            result.stdout.splitlines(),
            [
                str(self.irohad_path),
                "--sora",
                "--config",
                str(runtime_config_path),
                "--genesis-manifest-json",
                str(genesis_path),
            ],
        )

    def test_signed_genesis_override_updates_runtime_config(self) -> None:
        config_path = self.temp_path / "config.toml"
        runtime_config_path = self.temp_path / "runtime-config.toml"
        genesis_path = self.temp_path / "genesis.json"
        signed_genesis_path = self.temp_path / "genesis.signed.nrt"
        config_path.write_text(
            "\n".join(
                [
                    "chain = \"taira\"",
                    "",
                    "[genesis]",
                    "public_key = \"ed0120DEADBEEF\"",
                    "",
                    "[logger]",
                    "level = \"info\"",
                    "",
                ]
            ),
            encoding="utf-8",
        )
        genesis_path.write_text("{}\n", encoding="utf-8")
        signed_genesis_path.write_bytes(b"norito")

        result = self._run(
            env={
                "IROHA_IMAGE_CONFIG_PROFILE": "taira",
                "IROHA_TAIRA_CONFIG": str(config_path),
                "IROHA_TAIRA_RUNTIME_CONFIG": str(runtime_config_path),
                "IROHA_TAIRA_GENESIS": str(genesis_path),
                "IROHA_TAIRA_SIGNED_GENESIS": str(signed_genesis_path),
                "TAIRA_RUNTIME_PROFILE": "localnet",
            }
        )

        self.assertEqual(result.returncode, 0)
        rendered = runtime_config_path.read_text(encoding="utf-8")
        self.assertIn(f'file = "{signed_genesis_path}"', rendered)
        self.assertIn('public_key = "ed0120DEADBEEF"', rendered)
        self.assertEqual(
            result.stdout.splitlines(),
            [
                str(self.irohad_path),
                "--sora",
                "--config",
                str(runtime_config_path),
                "--genesis-manifest-json",
                str(genesis_path),
            ],
        )

    def test_explicit_command_overrides_profile_defaults(self) -> None:
        result = self._run(
            "bash",
            "-lc",
            "printf 'override\\n'",
            env={"IROHA_IMAGE_CONFIG_PROFILE": "taira"},
        )

        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stdout, "override\n")

    def test_taira_mode_requires_rendered_config(self) -> None:
        genesis_path = self.temp_path / "genesis.json"
        genesis_path.write_text("{}\n", encoding="utf-8")

        result = self._run(
            env={
                "IROHA_IMAGE_CONFIG_PROFILE": "taira",
                "IROHA_TAIRA_CONFIG": str(self.temp_path / "missing-config.toml"),
                "IROHA_TAIRA_GENESIS": str(genesis_path),
            }
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing Taira config", result.stderr)

    def test_production_requires_immutable_image_reference(self) -> None:
        config_path = self.temp_path / "config.toml"
        runtime_config_path = self.temp_path / "runtime-config.toml"
        genesis_path = self.temp_path / "genesis.json"
        config_path.write_text('chain = "taira"\n', encoding="utf-8")
        genesis_path.write_text("{}\n", encoding="utf-8")

        result = self._run(
            env={
                "IROHA_IMAGE_CONFIG_PROFILE": "taira",
                "IROHA_TAIRA_CONFIG": str(config_path),
                "IROHA_TAIRA_RUNTIME_CONFIG": str(runtime_config_path),
                "IROHA_TAIRA_GENESIS": str(genesis_path),
                "TAIRA_RUNTIME_PROFILE": "production",
            }
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("immutable image ID or repository@sha256 digest", result.stderr)

    def test_production_requires_and_accepts_complete_configured_assets(self) -> None:
        bundle_path = self.temp_path / "bundle"
        runtime_assets = bundle_path / "runtime"
        manifests = bundle_path / "manifests"
        admission = bundle_path / "sorafs_admission"
        for directory in (
            runtime_assets,
            manifests,
            admission,
        ):
            directory.mkdir(parents=True, exist_ok=True)
        onboarding_signer = runtime_assets / "onboarding-signer.key"
        faucet_signer = runtime_assets / "faucet-signer.key"
        governance_manifest = manifests / "governance.manifest.json"
        onboarding_signer.write_text("onboarding\n", encoding="utf-8")
        faucet_signer.write_text("faucet\n", encoding="utf-8")
        governance_manifest.write_text("{}\n", encoding="utf-8")

        config_path = bundle_path / "config.toml"
        runtime_config_path = self.temp_path / "storage" / "runtime-config.toml"
        genesis_path = self.temp_path / "genesis.json"
        config_path.write_text(
            "\n".join(
                [
                    'chain = "taira"',
                    "",
                    "[torii.account_onboarding]",
                    f'private_key_file = "{onboarding_signer}"',
                    "",
                    "[torii.faucet]",
                    f'private_key_file = "{faucet_signer}"',
                    "",
                    "[sorafs.discovery.admission]",
                    f'envelopes_dir = "{admission}"',
                    "",
                    "[nexus.registry]",
                    f'manifest_directory = "{manifests}"',
                    f'cache_directory = "{manifests}"',
                    "",
                ]
            ),
            encoding="utf-8",
        )
        genesis_path.write_text("{}\n", encoding="utf-8")
        image_reference = f"example/taira@sha256:{'a' * 64}"

        result = self._run(
            env={
                "IROHA_IMAGE_CONFIG_PROFILE": "taira",
                "IROHA_TAIRA_CONFIG": str(config_path),
                "IROHA_TAIRA_RUNTIME_CONFIG": str(runtime_config_path),
                "IROHA_TAIRA_GENESIS": str(genesis_path),
                "TAIRA_RUNTIME_PROFILE": "production",
                "TAIRA_IMAGE_REFERENCE": image_reference,
            }
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            runtime_config_path.read_text(encoding="utf-8"),
            config_path.read_text(encoding="utf-8"),
        )
        validation_only = self._run(
            "--validate-taira-production-config",
            str(config_path),
            env={},
        )
        self.assertEqual(validation_only.returncode, 0, validation_only.stderr)
        self.assertEqual(validation_only.stdout, "")

        self.assertNotIn("Kagemusha", result.stderr)

    def test_runtime_config_update_replaces_symlink_without_following_it(self) -> None:
        config_path = self.temp_path / "config.toml"
        runtime_config_path = self.temp_path / "runtime-config.toml"
        victim_path = self.temp_path / "victim"
        genesis_path = self.temp_path / "genesis.json"
        config_path.write_text('chain = "taira"\n', encoding="utf-8")
        victim_path.write_text("do not overwrite\n", encoding="utf-8")
        runtime_config_path.symlink_to(victim_path)
        genesis_path.write_text("{}\n", encoding="utf-8")

        result = self._run(
            env={
                "IROHA_IMAGE_CONFIG_PROFILE": "taira",
                "IROHA_TAIRA_CONFIG": str(config_path),
                "IROHA_TAIRA_RUNTIME_CONFIG": str(runtime_config_path),
                "IROHA_TAIRA_GENESIS": str(genesis_path),
                "TAIRA_RUNTIME_PROFILE": "localnet",
            }
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(victim_path.read_text(encoding="utf-8"), "do not overwrite\n")
        self.assertFalse(runtime_config_path.is_symlink())
        self.assertEqual(
            runtime_config_path.read_text(encoding="utf-8"),
            config_path.read_text(encoding="utf-8"),
        )


if __name__ == "__main__":
    unittest.main()
