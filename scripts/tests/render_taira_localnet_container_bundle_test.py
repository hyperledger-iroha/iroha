"""Tests for rendering container-ready Taira localnet bundles."""

from __future__ import annotations

import re
import subprocess
import tempfile
import textwrap
import unittest
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - exercised on Python 3.9/3.10
    import tomli as tomllib


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT_PATH = REPO_ROOT / "scripts" / "render_taira_localnet_container_bundle.py"


class RenderTairaLocalnetContainerBundleTest(unittest.TestCase):
    """Validate the localnet-to-container renderer against kagami-style fixtures."""

    def setUp(self) -> None:
        self.tempdir = tempfile.TemporaryDirectory()
        self.root = Path(self.tempdir.name)
        self.bundle_dir = self.root / "bundle"
        self.bundle_dir.mkdir()
        (self.bundle_dir / "genesis.json").write_text("{}\n", encoding="utf-8")
        (self.bundle_dir / "genesis.signed.nrt").write_bytes(b"norito")
        for peer_index in range(4):
            public_port = 1337 + peer_index
            torii_port = 8080 + peer_index
            peer_config = textwrap.dedent(
                f"""\
                chain = "iroha3-taira"
                public_key = "peer{peer_index}-pub"

                [genesis]
                file = "{self.bundle_dir}/genesis.signed.nrt"
                public_key = "genesis-pub"

                [kura]
                store_dir = "{self.bundle_dir}/storage/peer{peer_index}"

                [network]
                address = "addr:0.0.0.0:{public_port}#net{peer_index}"
                public_address = "addr:127.0.0.1:{public_port}#pub{peer_index}"
                trusted_peers = ["placeholder"]

                [tiered_state]
                cold_store_root = "{self.bundle_dir}/storage/peer{peer_index}/tiered_state"
                da_store_root = "{self.bundle_dir}/storage/peer{peer_index}/da_wsv_snapshots"

                [torii]
                address = "addr:0.0.0.0:{torii_port}#torii{peer_index}"
                peer_telemetry_urls = ["http://127.0.0.1:8080/"]
                """
            )
            (self.bundle_dir / f"peer{peer_index}.toml").write_text(peer_config, encoding="utf-8")
        self.output_dir = self.root / "rendered"
        self.install_runtime_pop_rosters(self.runtime_roster(4))

    def tearDown(self) -> None:
        self.tempdir.cleanup()

    def install_runtime_pop_rosters(
        self,
        entries: list[tuple[str, str]],
        *,
        peer_overrides: dict[int, list[tuple[str, str]]] | None = None,
    ) -> None:
        """Promote the synthetic fixtures to runtime-shaped roster configs."""

        peer_overrides = peer_overrides or {}
        for peer_index in range(4):
            config_path = self.bundle_dir / f"peer{peer_index}.toml"
            content = config_path.read_text(encoding="utf-8")
            content = re.sub(
                r"^trusted_peers_pop = \[\n.*?^\]\n",
                "",
                content,
                count=1,
                flags=re.MULTILINE | re.DOTALL,
            )
            if not re.search(r'^trusted_peers = \["placeholder"\]$', content, re.MULTILINE):
                raise AssertionError("fixture must retain one trusted_peers assignment")
            network_marker = (
                '[network]\n'
                f'address = "addr:0.0.0.0:{1337 + peer_index}#net{peer_index}"\n'
                f'public_address = "addr:127.0.0.1:{1337 + peer_index}#pub{peer_index}"\n'
                'trusted_peers = ["placeholder"]\n'
            )
            if network_marker in content:
                content = content.replace(
                    network_marker,
                    network_marker.removesuffix('trusted_peers = ["placeholder"]\n'),
                    1,
                )
                content = content.replace(
                    f'public_key = "peer{peer_index}-pub"\n',
                    f'public_key = "peer{peer_index}-pub"\n'
                    'trusted_peers = ["placeholder"]\n',
                    1,
                )
            roster = peer_overrides.get(peer_index, entries)
            rendered_roster = ",\n".join(
                "  { public_key = "
                f'"{public_key}", pop_hex = "{pop_hex}" }}'
                for public_key, pop_hex in roster
            )
            top_level_roster = "trusted_peers_pop = [\n" f"{rendered_roster}\n" "]\n"
            content = content.replace(
                'trusted_peers = ["placeholder"]\n',
                'trusted_peers = ["placeholder"]\n' + top_level_roster,
                1,
            )
            config_path.write_text(content, encoding="utf-8")

    def remove_runtime_pop_rosters(self) -> None:
        for peer_index in range(4):
            config_path = self.bundle_dir / f"peer{peer_index}.toml"
            content = re.sub(
                r"^trusted_peers_pop = \[\n.*?^\]\n",
                "",
                config_path.read_text(encoding="utf-8"),
                count=1,
                flags=re.MULTILINE | re.DOTALL,
            )
            config_path.write_text(content, encoding="utf-8")

    @staticmethod
    def runtime_roster(size: int) -> list[tuple[str, str]]:
        return [
            (f"peer{index}-pub", f"{index + 1:02x}" * 96)
            for index in range(size)
        ]

    def run_script(
        self,
        *extra_args: str,
    ) -> subprocess.CompletedProcess[str]:
        command = [
            "python3",
            str(SCRIPT_PATH),
            "--bundle-dir",
            str(self.bundle_dir),
            "--output-dir",
            str(self.output_dir),
        ]
        command.extend(extra_args)
        return subprocess.run(
            command,
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
            check=False,
        )

    def test_renderer_outputs_container_ready_configs_and_env_files(self) -> None:
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stderr)

        rendered_config = (self.output_dir / "peer0" / "config.toml").read_text(
            encoding="utf-8"
        )
        self.assertRegex(rendered_config, r'address = "addr:0\.0\.0\.0:1337#[0-9A-F]{4}"')
        self.assertRegex(
            rendered_config,
            r'public_address = "addr:taira-localnet-peer0:1337#[0-9A-F]{4}"',
        )
        self.assertRegex(
            rendered_config,
            r'trusted_peers = \["peer0-pub@addr:taira-localnet-peer0:1337#[0-9A-F]{4}"',
        )
        self.assertRegex(
            rendered_config,
            r'"peer3-pub@addr:taira-localnet-peer3:1337#[0-9A-F]{4}"\]',
        )
        self.assertIn('file = "/config/genesis.signed.nrt"', rendered_config)
        self.assertIn('store_dir = "/storage/kura"', rendered_config)
        self.assertIn('cold_store_root = "/storage/tiered_state"', rendered_config)
        self.assertIn('da_store_root = "/storage/da_wsv_snapshots"', rendered_config)
        self.assertRegex(rendered_config, r'address = "addr:0\.0\.0\.0:8080#[0-9A-F]{4}"')
        self.assertIn(
            'peer_telemetry_urls = ["http://taira-localnet-peer0:8080/", "http://taira-localnet-peer1:8080/", '
            '"http://taira-localnet-peer2:8080/", "http://taira-localnet-peer3:8080/"]',
            rendered_config,
        )

        rendered_env = (self.output_dir / "peer0.env").read_text(encoding="utf-8")
        self.assertIn("TAIRA_CONTAINER_NAME=taira-localnet-peer0", rendered_env)
        self.assertIn("TAIRA_IMAGE=local/taira-validator:prebuilt", rendered_env)
        self.assertIn("TAIRA_RUNTIME_PROFILE=localnet", rendered_env)
        self.assertIn(
            f"TAIRA_CONFIG_BUNDLE_PATH={(self.output_dir / 'peer0').resolve()}",
            rendered_env,
        )
        self.assertIn("TAIRA_P2P_PORT=31337", rendered_env)
        self.assertIn("TAIRA_TORII_PORT=28080", rendered_env)
        self.assertIn("TAIRA_DOCKER_NETWORK=taira-localnet", rendered_env)
        self.assertIn("TAIRA_SIGNED_GENESIS_PATH=", rendered_env)

    def test_runtime_renderer_preserves_exact_four_peer_roster(self) -> None:
        roster = self.runtime_roster(4)
        self.install_runtime_pop_rosters(roster)

        result = self.run_script()

        self.assertEqual(result.returncode, 0, result.stderr)
        expected_public_keys = [public_key for public_key, _pop_hex in roster]
        for peer_index in range(4):
            rendered_path = self.output_dir / f"peer{peer_index}" / "config.toml"
            with rendered_path.open("rb") as handle:
                rendered = tomllib.load(handle)
            self.assertEqual(
                [entry.partition("@")[0] for entry in rendered["trusted_peers"]],
                expected_public_keys,
            )
            self.assertEqual(
                {
                    entry["public_key"]: entry["pop_hex"]
                    for entry in rendered["trusted_peers_pop"]
                },
                dict(roster),
            )

    def test_runtime_renderer_rejects_seven_to_four_roster_truncation(self) -> None:
        self.install_runtime_pop_rosters(self.runtime_roster(7))

        result = self.run_script()

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "trusted_peers_pop` public-key set must exactly match discovered peer config public keys",
            result.stderr,
        )
        self.assertIn("peer4-pub", result.stderr)

    def test_runtime_renderer_rejects_missing_and_duplicate_pop_entries(self) -> None:
        cases = {
            "missing": (self.runtime_roster(3), "missing=['peer3-pub']"),
            "duplicate": (
                self.runtime_roster(4) + [self.runtime_roster(4)[0]],
                "contains duplicate public key `peer0-pub`",
            ),
        }
        for label, (roster, expected) in cases.items():
            with self.subTest(label=label):
                self.install_runtime_pop_rosters(roster)
                result = self.run_script()
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(expected, result.stderr)

    def test_runtime_renderer_rejects_divergent_pop_rosters(self) -> None:
        roster = self.runtime_roster(4)
        divergent = list(roster)
        divergent[0] = (divergent[0][0], "ff" * 96)
        self.install_runtime_pop_rosters(roster, peer_overrides={3: divergent})

        result = self.run_script()

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("must carry an identical `trusted_peers_pop` roster", result.stderr)

    def test_runtime_renderer_requires_pop_roster(self) -> None:
        self.remove_runtime_pop_rosters()

        result = self.run_script()

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("runtime localnet peer configs must define", result.stderr)

    def test_renderer_requires_four_peers(self) -> None:
        (self.bundle_dir / "peer3.toml").unlink()
        result = self.run_script()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("must contain at least 4 peer*.toml files", result.stderr)

    def test_renderer_rejects_noncontiguous_peer_indices(self) -> None:
        (self.bundle_dir / "peer3.toml").rename(self.bundle_dir / "peer4.toml")

        result = self.run_script()

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "localnet peer indices must be contiguous and start at zero",
            result.stderr,
        )

    def test_renderer_rejects_host_port_range_overflow(self) -> None:
        result = self.run_script("--base-torii-port", "65534")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "base Torii port range must remain within 1..65535",
            result.stderr,
        )

    def test_renderer_rejects_overlapping_host_port_ranges(self) -> None:
        result = self.run_script(
            "--base-p2p-port",
            "28080",
            "--base-torii-port",
            "28082",
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "localnet host P2P and Torii port ranges must not overlap",
            result.stderr,
        )


if __name__ == "__main__":
    unittest.main()
