"""Tests for scripts/docker_entrypoint.sh."""

from __future__ import annotations

import os
import stat
import subprocess
import tempfile
import unittest
from pathlib import Path


ENTRYPOINT_PATH = Path(__file__).resolve().parents[1] / "docker_entrypoint.sh"
TAIRA_RUNTIME_SIGNER_PATH = "/run/secrets/iroha-taira-runtime-signer.private_key"
TAIRA_RUNTIME_SIGNER_LAUNCH_PATH = "/storage/private/taira-runtime-signer.fd198"


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
        self.taira_path = self.bin_dir / "iroha3d_taira"
        self.taira_path.write_text(
            "#!/usr/bin/env bash\n"
            "signer_bytes=$(wc -c <&198 | tr -d '[:space:]')\n"
            "printf '%s\\n' \"$0\" \"$@\" \"fd198-bytes=$signer_bytes\"\n",
            encoding="utf-8",
        )
        self.taira_path.chmod(self.taira_path.stat().st_mode | stat.S_IEXEC)
        self.runtime_signer_path = self.temp_path / "runtime-signer.private_key"
        self.runtime_signer_path.write_bytes(b"x" * 71)
        self.runtime_signer_path.chmod(0o600)
        self.runtime_signer_launch_path = (
            self.temp_path / "private" / "taira-runtime-signer.fd198"
        )
        self.entrypoint_path = self.temp_path / "docker_entrypoint.sh"
        entrypoint_source = (
            ENTRYPOINT_PATH.read_text(encoding="utf-8")
            .replace(
                TAIRA_RUNTIME_SIGNER_PATH,
                str(self.runtime_signer_path),
            )
            .replace(
                TAIRA_RUNTIME_SIGNER_LAUNCH_PATH,
                str(self.runtime_signer_launch_path),
            )
        )
        self.entrypoint_path.write_text(entrypoint_source, encoding="utf-8")
        self.entrypoint_path.chmod(
            self.entrypoint_path.stat().st_mode | stat.S_IEXEC
        )

    def _run(self, *args: str, env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
        full_env = os.environ.copy()
        full_env["PATH"] = f"{self.bin_dir}:{full_env['PATH']}"
        if env:
            full_env.update(env)
        return subprocess.run(
            [str(self.entrypoint_path), *args],
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
            }
        )

        self.assertEqual(result.returncode, 0)
        self.assertEqual(
            runtime_config_path.read_text(encoding="utf-8"),
            config_path.read_text(encoding="utf-8"),
        )
        self.assertEqual(
            result.stdout.splitlines(),
            [
                str(self.taira_path),
                "--sora",
                "--config",
                str(runtime_config_path),
                "--genesis-manifest-json",
                str(genesis_path),
                "fd198-bytes=71",
            ],
        )
        self.assertEqual(self.runtime_signer_path.read_bytes(), b"x" * 71)
        self.assertEqual(self.runtime_signer_launch_path.read_bytes(), b"x" * 71)
        self.assertFalse(
            self.runtime_signer_path.samefile(self.runtime_signer_launch_path)
        )
        self.assertEqual(
            self.runtime_signer_launch_path.stat().st_mode & 0o7777, 0o600
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
                str(self.taira_path),
                "--sora",
                "--config",
                str(runtime_config_path),
                "--genesis-manifest-json",
                str(genesis_path),
                "fd198-bytes=71",
            ],
        )

    def test_taira_mode_requires_the_fixed_runtime_signer(self) -> None:
        config_path = self.temp_path / "config.toml"
        runtime_config_path = self.temp_path / "runtime-config.toml"
        genesis_path = self.temp_path / "genesis.json"
        config_path.write_text('chain = "taira"\n', encoding="utf-8")
        genesis_path.write_text("{}\n", encoding="utf-8")
        self.runtime_signer_path.unlink()

        result = self._run(
            env={
                "IROHA_IMAGE_CONFIG_PROFILE": "taira",
                "IROHA_TAIRA_CONFIG": str(config_path),
                "IROHA_TAIRA_RUNTIME_CONFIG": str(runtime_config_path),
                "IROHA_TAIRA_GENESIS": str(genesis_path),
            }
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing Taira runtime signer", result.stderr)
        self.assertFalse(runtime_config_path.exists())

    def test_checked_in_taira_entrypoint_has_one_fixed_signer_path(self) -> None:
        source = ENTRYPOINT_PATH.read_text(encoding="utf-8")

        self.assertEqual(source.count(TAIRA_RUNTIME_SIGNER_PATH), 1)
        self.assertEqual(source.count(TAIRA_RUNTIME_SIGNER_LAUNCH_PATH), 1)
        self.assertIn('exec 198<>"$runtime_signer_launch_path"', source)
        self.assertNotIn('exec 198<"$runtime_signer_path"', source)
        self.assertIn('cp "$runtime_signer_path" "$runtime_signer_tmp"', source)
        self.assertIn("exec iroha3d_taira --sora", source)
        self.assertNotIn("IROHA_TAIRA_RUNTIME_SIGNER", source)

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
