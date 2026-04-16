"""Tests for rendering container-ready Taira localnet bundles."""

from __future__ import annotations

import subprocess
import tempfile
import textwrap
import unittest
from pathlib import Path


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

    def tearDown(self) -> None:
        self.tempdir.cleanup()

    def run_script(self, *extra_args: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                "python3",
                str(SCRIPT_PATH),
                "--bundle-dir",
                str(self.bundle_dir),
                "--output-dir",
                str(self.output_dir),
                *extra_args,
            ],
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
            check=False,
        )

    def test_renderer_outputs_container_ready_configs_and_env_files(self) -> None:
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stderr)

        rendered_config = (self.output_dir / "peer0.toml").read_text(encoding="utf-8")
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
        self.assertIn("TAIRA_P2P_PORT=31337", rendered_env)
        self.assertIn("TAIRA_TORII_PORT=28080", rendered_env)
        self.assertIn("TAIRA_DOCKER_NETWORK=taira-localnet", rendered_env)
        self.assertIn("TAIRA_SIGNED_GENESIS_PATH=", rendered_env)

    def test_renderer_requires_four_peers(self) -> None:
        (self.bundle_dir / "peer3.toml").unlink()
        result = self.run_script()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("must contain at least 4 peer*.toml files", result.stderr)


if __name__ == "__main__":
    unittest.main()
