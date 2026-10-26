"""Static safety tests for scripts/deploy_localnet.sh."""

from __future__ import annotations

import subprocess
import os
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "deploy_localnet.sh"


class DeployLocalnetShellSafetyTest(unittest.TestCase):
    def test_current_generator_keeps_committed_fee_policy_out_of_peer_config(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            generator = root / "kagami"
            generator.write_text('''#!/usr/bin/env python3
from pathlib import Path
import sys
assert sys.argv[1:3] == ['localnet', 'generate']
assert sys.argv[sys.argv.index('--block-cadence-ms') + 1] == '5000'
root = Path(sys.argv[sys.argv.index('--out-dir') + 1])
root.mkdir()
(root / 'peer0.toml').write_text('[pipeline]\\nivm_execution_max_bytes = 16777216\\n')
(root / 'start.sh').write_text('#!/bin/sh\\nexit 92\\n')
(root / 'stop.sh').write_text('#!/bin/sh\\nexit 0\\n')
''')
            generator.chmod(0o700)
            output = root / 'network'
            result = subprocess.run(
                ['bash', str(SCRIPT), '--iroha-dir', str(REPO_ROOT),
                 '--out-dir', str(output), '--no-build', '--block-cadence-ms', '5000'],
                env={**os.environ, 'KAGAMI_BIN': str(generator),
                     'IROHAD_BIN': '/usr/bin/true', 'IROHA_BIN': '/usr/bin/true'},
                capture_output=True, text=True, timeout=30,
            )
            self.assertEqual(result.returncode, 92, result.stdout + result.stderr)
            self.assertEqual((output / 'peer0.toml').read_text(),
                             '[pipeline]\nivm_execution_max_bytes = 16777216\n')

    def test_script_parses(self) -> None:
        subprocess.run(["bash", "-n", str(SCRIPT)], check=True)

    def test_builtin_xor_asset_verification_cannot_be_skipped(self) -> None:
        text = SCRIPT.read_text(encoding="utf-8")
        self.assertNotIn("SKIP_ASSET_CHECK", text)
        self.assertIn("Verifying built-in XOR fee alias", text)
        for flag in ("--skip-asset-check", "--skip-asset-register"):
            self.assertNotIn(flag, text)
            rejected = subprocess.run(
                ["bash", str(SCRIPT), flag], capture_output=True, text=True, timeout=10
            )
            self.assertEqual(rejected.returncode, 2)
            self.assertIn(f"Unknown option: {flag}", rejected.stderr)
        for helper in ("run_10k_localnet.sh", "run_100tps_profile_localnet.sh", "run_localnet_memory_guard.py"):
            self.assertNotIn("--skip-asset-register", (REPO_ROOT / "scripts" / helper).read_text(encoding="utf-8"))

    def test_force_cleanup_uses_guarded_pid_ownership_checks(self) -> None:
        text = SCRIPT.read_text(encoding="utf-8")
        self.assertIn("pid_matches_localnet_peer()", text)
        self.assertIn("pid_is_running()", text)
        self.assertIn("command -v ps >/dev/null 2>&1 || return 0", text)
        self.assertIn("stop_existing_localnet()", text)
        self.assertIn('grep -F -- "--config $config_path"', text)
        self.assertIn('kill "$pid" 2>/dev/null || true', text)
        self.assertNotIn("kill -0", text)
        self.assertNotIn('kill -9 "$pid"', text)
        self.assertNotIn("kill -KILL", text)
        self.assertNotIn("&& ./stop.sh 2>/dev/null", text)
        self.assertIn(
            "Stopping existing Iroha peers in $OUT_DIR with guarded pid ownership checks",
            text,
        )
        self.assertIn(
            "Out-dir $OUT_DIR still has live or mismatched pidfiles; not removing it.",
            text,
        )
        self.assertIn(
            "Refusing to remove existing out-dir while localnet peer",
            text,
        )
        self.assertIn(
            "live pid $pid does not match $config_path",
            text,
        )

    def test_10k_perf_profile_caps_kura_hot_cache(self) -> None:
        text = SCRIPT.read_text(encoding="utf-8")
        self.assertIn("--kura-blocks-in-memory <N>", text)
        self.assertIn("IROHA_LOCALNET_KURA_BLOCKS_IN_MEMORY", text)
        self.assertIn("10k-*)", text)
        self.assertIn("KURA_BLOCKS_IN_MEMORY=32", text)
        self.assertIn("blocks_in_memory = \" blocks", text)

    def test_peer_count_fails_closed_to_sumeragi_committee_geometry(self) -> None:
        text = SCRIPT.read_text(encoding="utf-8")
        self.assertIn("PEERS < 4 || PEERS > 31", text)
        self.assertIn("(PEERS - 1) % 3 != 0", text)
        self.assertIn("exact Sumeragi 3f + 1 committee", text)

    def test_cadence_override_uses_the_signed_genesis_block_cadence_flag(self) -> None:
        text = SCRIPT.read_text(encoding="utf-8")
        self.assertIn("--block-cadence-ms)", text)
        self.assertIn('KAGAMI_ARGS+=(--block-cadence-ms "$BLOCK_CADENCE_MS")', text)

    def test_tool_binaries_share_one_cargo_build_invocation(self) -> None:
        text = SCRIPT.read_text(encoding="utf-8")
        start = text.index("build_tool_bins() {")
        end = text.index("\n}", start)
        build_function = text[start:end]

        self.assertEqual(build_function.count('"${cargo_runner[@]}" build'), 1)
        for package_and_binary in (
            "-p iroha_kagami --bin kagami",
            "-p irohad --bin iroha3d",
            "-p iroha_cli --bin iroha",
        ):
            self.assertIn(package_and_binary, build_function)


if __name__ == "__main__":
    unittest.main()
