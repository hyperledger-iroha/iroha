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
        self.irohad_path = self.bin_dir / "irohad"
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

    def test_defaults_to_plain_irohad_for_non_taira_profiles(self) -> None:
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


if __name__ == "__main__":
    unittest.main()
